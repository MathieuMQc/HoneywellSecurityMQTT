#ifndef __DIGITAL_DECODER_H__
#define __DIGITAL_DECODER_H__

#include "mqtt.h"

#include <stdint.h>
#include <map>

class DigitalDecoder
{
  public:
    DigitalDecoder(Mqtt &mqtt_init) : mqtt(mqtt_init) {}
    
    void handleData(char data);
    void setRxGood(bool state);

    
  private:

    void writeDeviceState();
    void sendDeviceState();
    void updateDeviceState(uint32_t serial, uint8_t state);
    void handlePayload(uint64_t payload);
    void handleBit(bool value);
    void decodeBit(bool value);
    void checkForTimeouts();

    unsigned int samplesSinceEdge = 0;
    bool lastSample = false;
    bool rxGood = false;
    uint64_t lastRxGoodUpdateTime = 0;
    Mqtt &mqtt;
    uint32_t packetCount = 0;
    uint32_t errorCount = 0;
    
    struct deviceState_t
    {
        uint64_t lastUpdateTime;
        uint64_t lastAlarmTime;
        
        uint8_t lastRawState;
        
        bool tamper;
        bool alarm;
        bool batteryLow;
        bool timeout;
        
        uint8_t minAlarmStateSeen;
    };

    std::map<uint32_t, deviceState_t> deviceStateMap;
};

#endif
