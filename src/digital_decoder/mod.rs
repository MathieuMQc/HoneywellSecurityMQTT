pub mod mqtt_;
pub mod decoder {
    use std::time::SystemTime;

    use digital_decoder::mqtt_::mqtt::Mqtt;

    // Pulse checks seem to be about 60-70 minutes apart
    const RX_TIMEOUT_MIN: usize = 90;

    // Give each sensor 3 intervals before we flag a problem
    const SENSOR_TIMEOUT_MIN: usize = (90 * 5);

    const SYNC_MASK: u64 = 0xFFFF_0000_0000_0000;
    const SYNC_PATTERN: u64 = 0xFFFE_0000_0000_0000;

    // Don't send these messages more than once per minute unless there is a state change
    const RX_GOOD_MIN_SEC: u64 = (60);
    const UPDATE_MIN_SEC: usize = (60);

    const BASE_TOPIC: &str = "/security/sensors345/";

    enum ManchesterState {
        LOW_PHASE_A,
        LOW_PHASE_B,
        HIGH_PHASE_A,
        HIGH_PHASE_B,
    }

    struct DeviceState {
        last_update_time: u64,
        last_alarm_time: u64,

        last_raw_state: u8,

        tamper: bool,
        alarm: bool,
        battery_low_l: bool,
        timeout: bool,

        min_alarm_state_seen: u8,
    }

    pub struct DigitalDecoder<'a> {
        samples_since_edge: u32,
        last_sample: bool,
        rx_good: bool,
        last_rx_good_update_time: u64,
        mqtt: Mqtt<'a>,
        packet_count: u32,
        error_count: u32,
        // std::map<uint32_t, deviceState_t> deviceStateMap;
        //device_state: DeviceState,
        //state: ManchesterState,
    }

    impl<'a> DigitalDecoder<'a> {
        pub fn new() -> DigitalDecoder<'a> {
            DigitalDecoder {
                samples_since_edge: 0,
                last_sample: false,
                rx_good: false,
                last_rx_good_update_time: 0,
                mqtt: Mqtt::new(),
                packet_count: 0,
                error_count: 0,
                //device_state: DeviceState{},
                //state: ManchesterState::LOW_PHASE_A,
            }
        }

        pub fn handle_data(&mut self, data: u8) {
            let samples_per_bit = 8;

            if data != 0 && data != 1 {
                return;
            }

            let this_sample = data == 1;

            if this_sample == self.last_sample {
                self.samples_since_edge += 1;

                //if(samplesSinceEdge < 100)
                //{
                //    println!("At %d for %u\n", thisSample?1:0, samplesSinceEdge);
                //}

                if self.samples_since_edge % samples_per_bit == samples_per_bit / 2 {
                    // This Sample is a new bit
                    self.decode_bit(this_sample);
                }
            } else {
                self.samples_since_edge = 1;
            }
            self.last_sample = this_sample;
        }

        fn setRxGood(&mut self, state: bool) {
            let mut topic = BASE_TOPIC.to_owned();
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("Could not determine UNIX time")
                .as_secs();

            topic += "rx_status";

            if self.rx_good != state || (now - self.last_rx_good_update_time) > RX_GOOD_MIN_SEC {
                self.mqtt.send(topic, if state { "OK" } else { "FAILED" });
            }

            // Reset watchdog either way
            alarm(RX_TIMEOUT_MIN * 60);

            self.rx_good = state;
            self.last_rx_good_update_time = now;
        }

        // void writeDeviceState();

        // void sendDeviceState();

        // void updateDeviceState(uint32_t serial, uint8_t state);

        fn handlePayload(&mut self, payload: u64) {
            let sof = (payload & 0xF000_0000_0000) >> 44;
            let ser = (payload & 0x0FFF_FF00_0000) >> 24;
            let typ = (payload & 0x0000_00FF_0000) >> 16;
            let crc = (payload & 0x0000_0000_FFFF) >> 0;

            //
            // Check CRC
            //
            let polynomial: u64 = 0;
            if sof == 2 || sof == 10 {
                // 2GIG brand
                polynomial = 0x18050;
            } else {
                // sof == 0x8
                polynomial = 0x18005;
            }
            let sum = payload & (!SYNC_MASK);
            let current_divisor = polynomial << 31;

            while current_divisor >= polynomial {
                //#ifdef __arm__
                //        if(__builtin_clzll(sum) == __builtin_clzll(current_divisor))
                //#else
                //        if(__builtin_clzl(sum) == __builtin_clzl(current_divisor))
                //#endif
                {
                    sum ^= current_divisor;
                }
                current_divisor >>= 1;
            }

            let valid = sum == 0;

            // Print Packet
            //
            //#ifdef __arm__
            if valid {
                println!("Valid Payload: {}(Serial {}, Status {})", payload, ser, typ);
            } else {
                println!("Invalid Payload: {}", payload);
            } // #else
              //     if(valid)
              //         println!("Valid Payload: {} (Serial {}, Status {})", payload, ser, typ);
              //     else
              //         println!("Invalid Payload: {}", payload);
              // #endif

            self.packet_count += 1;
            if !valid {
                self.error_count += 1;
                println!(
                    "{}/{} packets failed CRC",
                    self.error_count, self.packet_count
                );
            }

            // Tell the world
            //
            if valid {
                // We received a valid packet so the receiver must be working
                self.setRxGood(true);
                // Update the device
                self.updateDeviceState(ser, typ);
            }
        }

        fn handle_bit(&mut self, value: bool) {
            static mut PAYLOAD: u64 = 0;
            unsafe {
                PAYLOAD <<= 1;
                if value {
                    PAYLOAD |= 1;
                } else {
                    PAYLOAD |= 0;
                }

                //#ifdef __arm__
                //    println!("Got bit: %d, payload is now %llX\n", value?1:0, payload);
                //#else
                //    println!("Got bit: %d, payload is now %lX\n", value?1:0, payload);
                //#endif

                if (PAYLOAD & SYNC_MASK) == SYNC_PATTERN {
                    self.handlePayload(PAYLOAD);
                    PAYLOAD = 0;
                }
            }
        }

        fn decode_bit(&mut self, value: bool) {
            static mut STATE: ManchesterState = ManchesterState::LOW_PHASE_A;
            unsafe {
                match STATE {
                    ManchesterState::LOW_PHASE_A => {
                        if value {
                            STATE = ManchesterState::HIGH_PHASE_B;
                        } else {
                            STATE = ManchesterState::LOW_PHASE_A;
                        }
                    }
                    ManchesterState::LOW_PHASE_B => {
                        self.handle_bit(false);
                        if value {
                            STATE = ManchesterState::HIGH_PHASE_A;
                        } else {
                            STATE = ManchesterState::LOW_PHASE_A;
                        }
                    }
                    ManchesterState::HIGH_PHASE_A => {
                        if value {
                            STATE = ManchesterState::HIGH_PHASE_A;
                        } else {
                            STATE = ManchesterState::LOW_PHASE_B;
                        }
                    }
                    ManchesterState::HIGH_PHASE_B => {
                        self.handle_bit(true);
                        if value {
                            STATE = ManchesterState::HIGH_PHASE_A;
                        } else {
                            STATE = ManchesterState::LOW_PHASE_A;
                        }
                    }
                }
            }
        }

        fn checkForTimeouts(&mut self) {
            //         timeval now;
            // std::ostringstream status;
            //
            // status << "TIMEOUT";
            // gettimeofday(&now, nullptr);
            //
            // for(const auto &dd : deviceStateMap)
            // {
            //     if ((now.tv_sec - dd.second.lastUpdateTime) > SENSOR_TIMEOUT_MIN*60)
            //     {
            //         if (false == dd.second.timeout)
            //         {
            //             std::ostringstream statusTopic;
            //
            //             deviceStateMap[dd.first].timeout = true;
            //             statusTopic << BASE_TOPIC << dd.first << "/status";
            //             mqtt.send(statusTopic.str().c_str(), status.str().c_str());
            //         }
            //     }
            //   }
        }
    }
}
