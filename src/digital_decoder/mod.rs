pub mod decoder {
    use std::collections::HashMap;
    use std::time::SystemTime;

    // Pulse checks seem to be about 60-70 minutes apart
    const RX_TIMEOUT_MIN: usize = 90;

    // Give each sensor 3 intervals before we flag a problem
    const SENSOR_TIMEOUT_MIN: u64 = (90 * 5);

    const SYNC_MASK: u64 = 0xFFFF_0000_0000_0000;
    const SYNC_PATTERN: u64 = 0xFFFE_0000_0000_0000;

    // Don't send these messages more than once per minute unless there is a state change
    const RX_GOOD_MIN_SEC: u64 = (60);
    const UPDATE_MIN_SEC: u64 = (60);

    const BASE_TOPIC: &str = "/security/sensors345/";

    enum ManchesterState {
        LOW_PHASE_A,
        LOW_PHASE_B,
        HIGH_PHASE_A,
        HIGH_PHASE_B,
    }

    #[derive(Copy, Clone, Debug)]
    struct DeviceState {
        last_update_time: u64,
        last_alarm_time: u64,

        last_raw_state: u8,

        tamper: bool,
        alarm: bool,
        battery_low: bool,
        timeout: bool,

        min_alarm_state_seen: u8,
    }

    pub struct DigitalDecoder {
        samples_since_edge: u32,
        last_sample: bool,
        rx_good: bool,
        last_rx_good_update_time: u64,
        //mqtt: Mqtt<'a>,
        packet_count: u32,
        error_count: u32,
        device_state_map: HashMap<u32, DeviceState>,
        device_state: DeviceState,
    }

    impl DeviceState {
        fn new() -> DeviceState {
            DeviceState {
                last_update_time: 0,
                last_alarm_time: 0,
                last_raw_state: 0,
                tamper: false,
                alarm: false,
                battery_low: false,
                timeout: false,
                min_alarm_state_seen: 0,
            }
        }
    }

    impl<'a> DigitalDecoder {
        pub fn new() -> DigitalDecoder {
            DigitalDecoder {
                samples_since_edge: 0,
                last_sample: false,
                rx_good: false,
                last_rx_good_update_time: 0,
                //mqtt: Mqtt::new(),
                packet_count: 0,
                error_count: 0,
                device_state: DeviceState::new(),
                device_state_map: HashMap::new(),
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

        fn set_rx_good(&mut self, state: bool) {
            let mut topic = BASE_TOPIC.to_owned();
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("Could not determine UNIX time")
                .as_secs();

            topic += "rx_status";

            if self.rx_good != state || (now - self.last_rx_good_update_time) > RX_GOOD_MIN_SEC {
                println!("Send {}", if state { "Ok" } else { "Failed" });
                //self.mqtt.send(topic, if state { "OK" } else { "FAILED" });
            }

            //            // Reset watchdog either way
            //            alarm(RX_TIMEOUT_MIN * 60);

            self.rx_good = state;
            self.last_rx_good_update_time = now;
        }

        fn update_device_state(&mut self, serial: u32, state: u8) -> () {
            let mut ds = DeviceState::new();
            let alarm_state: u8;
            let alarm_topic: String = BASE_TOPIC.to_owned() + &serial.to_string() + "/alarm";
            let status_topic: String = BASE_TOPIC.to_owned() + &serial.to_string() + "/status";

            // Extract prior info
            if self.device_state_map.contains_key(&serial) {
                ds = self
                    .device_state_map
                    .get(&serial)
                    .expect("Unable to find serial")
                    .clone();
            } else {
                ds.min_alarm_state_seen = 0xFF;
                ds.last_update_time = 0;
                ds.last_alarm_time = 0;
            }

            // Update minimum/OK state if needed
            // Look only at the non-tamper loop bits
            alarm_state = state & 176;
            if alarm_state < ds.min_alarm_state_seen {
                ds.min_alarm_state_seen = alarm_state;
            };

            // Decode alarm bits
            // We just alarm on any active loop that has been previously observed as inactive
            // This hopefully avoids having to use per-sensor configuration
            ds.alarm = alarm_state > ds.min_alarm_state_seen;

            // Decode tamper bit
            ds.tamper = state & 64 == 0;

            // Decode battery low bit
            ds.battery_low = state & 0x08 == 0;

            // Timestamp
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("Could not determine UNIX time")
                .as_secs();
            ds.timeout = false;

            if ds.alarm {
                ds.last_alarm_time = now
            };

            // Put the answer back in the map
            self.device_state_map.insert(serial, ds);

            // Send the notification if something changed or enough time has passed
            if state != ds.last_raw_state || (now - ds.last_update_time) > UPDATE_MIN_SEC {
                let mut status: String = "".to_string();

                // Send alarm state
                //mqtt.send(alarmTopic.str().c_str(), ds.alarm ? "ALARM" : "OK");
                println!("MQTT Send -> {} {}", alarm_topic, ds.alarm);

                // Build and send combined fault status
                if !ds.tamper && !ds.battery_low {
                    status = "OK".to_string();
                } else {
                    if ds.tamper {
                        status += "TAMPER ";
                    }

                    if ds.battery_low {
                        status += "LOWBATT";
                    }
                }
                //mqtt.send(statusTopic.str().c_str(), status.str().c_str());
                println!("MQTT Send -> {} {}", status_topic, status);

                let mut sm = self
                    .device_state_map
                    .get(&serial)
                    .expect("Unable to get")
                    .clone();
                sm.last_update_time = now;
                sm.last_raw_state = state;
                self.device_state_map.insert(serial, sm);

                &self
                    .device_state_map
                    .iter_mut()
                    .map(move |(_k, v)| {
                        v.last_alarm_time = now;
                        v.last_raw_state = state;
                    })
                    .collect::<()>();

                self.check_for_timeouts();

                for dd in &self.device_state_map {
                    println!(
                        "{}Device {}: {} {} {} {}\n",
                        if dd.0 == &serial { "*" } else { " " },
                        dd.0,
                        if dd.1.alarm { "ALARM" } else { "OK" },
                        if dd.1.tamper { "TAMPER" } else { "" },
                        if dd.1.battery_low { "LOWBATT" } else { "" },
                        if dd.1.timeout { "TIMEOUT" } else { "" }
                    );
                }
                println!();
            }
        }

        fn handle_payload(&mut self, payload: u64) {
            let sof = (payload & 0xF000_0000_0000) >> 44;
            let ser = (payload & 0x0FFF_FF00_0000) >> 24;
            let typ = (payload & 0x0000_00FF_0000) >> 16;
            let crc = (payload & 0x0000_0000_FFFF) >> 0;

            //
            // Check CRC
            //
            let polynomial: u64;
            if sof == 2 || sof == 10 {
                // 2GIG brand
                polynomial = 0x18050;
            } else {
                // sof == 0x8
                polynomial = 0x18005;
            }
            let mut sum = payload & (!SYNC_MASK);
            let mut current_divisor = polynomial << 31;

            while current_divisor >= polynomial {
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
                self.set_rx_good(true);
                // Update the device
                self.update_device_state(ser as u32, typ as u8);
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
                    self.handle_payload(PAYLOAD);
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

        fn check_for_timeouts(&mut self) {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("Could not determine UNIX time")
                .as_secs();

            let status: String = "TIMEOUT".to_string();

            &self
                .device_state_map
                .iter_mut()
                .filter(|(_k, v)| now - v.last_update_time > SENSOR_TIMEOUT_MIN * 60)
                .filter(|(_k, v)| v.timeout == false)
                .map(|(_k, v)| {
                    v.timeout = true;
                    //mqtt.send(statusTopic.str().c_str(), status.str().c_str());
                    println!("MQTT Send -> TIMEOUT {}", status);
                })
                .collect::<()>();
        }
    }
}
