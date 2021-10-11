#include "digitalDecoder.h"
#include "mqtt.h"

#include <iostream>
#include <fstream>
#include <string>
#include <sstream>
#include <sys/stat.h>
#include <sys/time.h>
#include <fcntl.h>
#include <unistd.h>
#include <stdint.h>
#include <csignal>


// Pulse checks seem to be about 60-70 minutes apart
#define RX_TIMEOUT_MIN      (90)

// Give each sensor 3 intervals before we flag a problem
#define SENSOR_TIMEOUT_MIN  (90*5)

#define SYNC_MASK    0xFFFF000000000000ul
#define SYNC_PATTERN 0xFFFE000000000000ul

// Don't send these messages more than once per minute unless there is a state change
#define RX_GOOD_MIN_SEC (60)
#define UPDATE_MIN_SEC (60)

static const char BASE_TOPIC[] = "/security/sensors345/";

void DigitalDecoder::setRxGood(bool state)
{
    std::string topic(BASE_TOPIC);
    timeval now;

    topic += "rx_status";

    gettimeofday(&now, nullptr);

    if (rxGood != state || (now.tv_sec - lastRxGoodUpdateTime) > RX_GOOD_MIN_SEC)
    {
        mqtt.send(topic.c_str(), state ? "OK" : "FAILED");
    }

    // Reset watchdog either way
    alarm(RX_TIMEOUT_MIN*60);

    rxGood = state;
    lastRxGoodUpdateTime = now.tv_sec;
}

void DigitalDecoder::updateDeviceState(uint32_t serial, uint8_t state)
{
    deviceState_t ds;
    uint8_t alarmState;
    std::ostringstream alarmTopic;
    std::ostringstream statusTopic;
    alarmTopic << BASE_TOPIC << serial << "/alarm";
    statusTopic << BASE_TOPIC << serial << "/status";
    
    // Extract prior info
    if(deviceStateMap.count(serial))
    {
        ds = deviceStateMap[serial];
    }
    else
    {
        ds.minAlarmStateSeen = 0xFF;
        ds.lastUpdateTime = 0;
        ds.lastAlarmTime = 0;
    }
    
    // Update minimum/OK state if needed
    // Look only at the non-tamper loop bits
    alarmState = (state & 0xB0);
    if(alarmState < ds.minAlarmStateSeen) ds.minAlarmStateSeen = alarmState; 
    
    // Decode alarm bits
    // We just alarm on any active loop that has been previously observed as inactive
    // This hopefully avoids having to use per-sensor configuration
    ds.alarm = (alarmState > ds.minAlarmStateSeen);
    
    // Decode tamper bit
    ds.tamper = (state & 0x40);
    
    // Decode battery low bit    
    ds.batteryLow = (state & 0x08);
    
    // Timestamp
    timeval now;
    gettimeofday(&now, nullptr);
    ds.timeout = false;
    
    if(ds.alarm) ds.lastAlarmTime = now.tv_sec;

    // Put the answer back in the map
    deviceStateMap[serial] = ds;
    
    // Send the notification if something changed or enough time has passed
    if(state != ds.lastRawState || (now.tv_sec - ds.lastUpdateTime) > UPDATE_MIN_SEC)
    {
        std::ostringstream status;

        // Send alarm state
        mqtt.send(alarmTopic.str().c_str(), ds.alarm ? "ALARM" : "OK");

        // Build and send combined fault status
        if (!ds.tamper && !ds.batteryLow)
        {
            status << "OK";
        } else {

            if (ds.tamper)
            {
                status << "TAMPER ";
            }
            
            if (ds.batteryLow)
            {
                status << "LOWBATT";
            }
        }
        mqtt.send(statusTopic.str().c_str(), status.str().c_str());

        deviceStateMap[serial].lastUpdateTime = now.tv_sec;
        deviceStateMap[serial].lastRawState = state;

        checkForTimeouts();

        for(const auto &dd : deviceStateMap)
        {
            printf("%sDevice %7u: %s %s %s %s\n",dd.first==serial ? "*" : " ", dd.first, dd.second.alarm ? "ALARM" : "OK", dd.second.tamper ? "TAMPER" : "", dd.second.batteryLow ? "LOWBATT" : "", dd.second.timeout ? "TIMEOUT" : "");
        }
        std::cout << std::endl;

    }
    
}

/* Checks all devices for last time updated */
void DigitalDecoder::checkForTimeouts()
{
    timeval now;
    std::ostringstream status;

    status << "TIMEOUT";
    gettimeofday(&now, nullptr);

    for(const auto &dd : deviceStateMap)
    {
        if ((now.tv_sec - dd.second.lastUpdateTime) > SENSOR_TIMEOUT_MIN*60) 
        {
            if (false == dd.second.timeout)
            {
                std::ostringstream statusTopic;

                deviceStateMap[dd.first].timeout = true;
                statusTopic << BASE_TOPIC << dd.first << "/status";
                mqtt.send(statusTopic.str().c_str(), status.str().c_str());
            }
        }
    }
}

void DigitalDecoder::handlePayload(uint64_t payload)
{
    uint64_t sof = (payload & 0xF00000000000) >> 44;
    uint64_t ser = (payload & 0x0FFFFF000000) >> 24;
    uint64_t typ = (payload & 0x000000FF0000) >> 16;
    uint64_t crc = (payload & 0x00000000FFFF) >>  0;
    
    //
    // Check CRC
    //
    uint64_t polynomial;
    if (sof == 0x2 || sof == 0xA) {
        // 2GIG brand
        polynomial = 0x18050;
    } else {
        // sof == 0x8
        polynomial = 0x18005;
    }
    uint64_t sum = payload & (~SYNC_MASK);
    uint64_t current_divisor = polynomial << 31;
    
    while(current_divisor >= polynomial)
    {
#ifdef __arm__
        if(__builtin_clzll(sum) == __builtin_clzll(current_divisor))
#else        
        if(__builtin_clzl(sum) == __builtin_clzl(current_divisor))
#endif
        {
            sum ^= current_divisor;
        }        
        current_divisor >>= 1;
    }
    
    const bool valid = (sum == 0);
    
    //
    // Print Packet
    //
 #ifdef __arm__
     if(valid)    
         printf("Valid Payload: %llX (Serial %llu, Status %llX)", payload, ser, typ);
     else
         printf("Invalid Payload: %llX", payload);
 #else    
     if(valid)    
         printf("Valid Payload: %lX (Serial %lu, Status %lX)", payload, ser, typ);
     else
         printf("Invalid Payload: %lX", payload);
 #endif
     std::cout << std::endl;
    
    packetCount++;
    if(!valid)
    {
        errorCount++;
        printf("%u/%u packets failed CRC", errorCount, packetCount);
        std::cout << std::endl;
    }

    //
    // Tell the world
    //
    if(valid)
    {
        // We received a valid packet so the receiver must be working
        setRxGood(true);
        // Update the device
        updateDeviceState(ser, typ);
    }
}



void DigitalDecoder::handleBit(bool value)
{
    static uint64_t payload = 0;
    
    payload <<= 1;
    payload |= (value ? 1 : 0);
    
//#ifdef __arm__
//    printf("Got bit: %d, payload is now %llX\n", value?1:0, payload);
//#else
//    printf("Got bit: %d, payload is now %lX\n", value?1:0, payload);
//#endif     
    
    if((payload & SYNC_MASK) == SYNC_PATTERN)
    {
        handlePayload(payload);
        payload = 0;
    }
}

void DigitalDecoder::decodeBit(bool value)
{
    enum ManchesterState
    {
        LOW_PHASE_A,
        LOW_PHASE_B,
        HIGH_PHASE_A,
        HIGH_PHASE_B
    };
    
    static ManchesterState state = LOW_PHASE_A;
    
    switch(state)
    {
        case LOW_PHASE_A:
        {
            state = value ? HIGH_PHASE_B : LOW_PHASE_A;
            break;
        }
        case LOW_PHASE_B:
        {
            handleBit(false);
            state = value ? HIGH_PHASE_A : LOW_PHASE_A;
            break;
        }
        case HIGH_PHASE_A:
        {
            state = value ? HIGH_PHASE_A : LOW_PHASE_B;
            break;
        }
        case HIGH_PHASE_B:
        {
            handleBit(true);
            state = value ? HIGH_PHASE_A : LOW_PHASE_A;
            break;
        }
    }
}

void DigitalDecoder::handleData(char data)
{
    static const int samplesPerBit = 8;
    
        
    if(data != 0 && data != 1) return;
        
    const bool thisSample = (data == 1);
    
    if(thisSample == lastSample)
    {
        samplesSinceEdge++;
        
        //if(samplesSinceEdge < 100)
        //{
        //    printf("At %d for %u\n", thisSample?1:0, samplesSinceEdge);
        //}
        
        if((samplesSinceEdge % samplesPerBit) == (samplesPerBit/2))
        {
            // This Sample is a new bit
            decodeBit(thisSample);
        }
    }
    else
    {
        samplesSinceEdge = 1;
    }
    lastSample = thisSample;
}
