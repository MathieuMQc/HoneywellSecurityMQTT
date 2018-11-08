pub mod mqtt {

    #[derive(Default)]
    pub struct Mqtt<'a> {
        pub host: &'a str,
        pub id: &'a str,
        pub port: i32,
        pub keepalive: i32,
        pub will_message: &'a str,
        pub will_topic: &'a str,
    }

    impl<'a> Mqtt<'a> {
        // pub fn new() -> Mqtt<'a> {
        //     Mqtt {
        //         host: "",
        //         id: "",
        //         port: 0,
        //         keepalive: 0,
        //         will_message: "",
        //         will_topic: "",
        //     }
        // }

fn new( _id:&str,  _host:&str,  _port:i32,  _username:&str,  _password:&str,  _will_topic:&str,  _will_message:&str)  //: mosquittopp(_id)
{
    int version = MQTT_PROTOCOL_V311;
    mosqpp::lib_init();
    this->keepalive = 30;    
    this->id = _id;
    this->port = _port;
    this->host = _host;
    this->will_topic = _will_topic;
    this->will_message = _will_message;
    // Set version to 3.1.1
    opts_set(MOSQ_OPT_PROTOCOL_VERSION, &version);
    // Set username and password if non-null
    if (strlen(_username) > 0 && strlen(_password) > 0) {
        username_pw_set(_username, _password);
    }
    // Set last will and testament (LWT) message
    if (will_topic != NULL && will_message != NULL) {
        int rc = set_will(will_topic, will_message);
        if ( rc ) {
            std::cout << ">> Mqtt - set LWT message to: " << will_message << std::endl;
        } else {
            std::cout << ">> Mqtt - Failed to set LWT message!" << std::endl;
        }
    }
    // non blocking connection to broker request;
    connect_async(host, port, keepalive);
    // Start thread managing connection / publish / subscribekeepalive);
    loop_start();
};

drop() {
    loop_stop();
    mosqpp::lib_cleanup();
}
fn set_will( topic: &str,  message:&str) -> bool
{
    let ret = will_set(topic, message.len(), message, 1, true);
      ret == MOSQ_ERR_SUCCESS 
}

fn on_disconnect( rc:i32) {
     println!( ">> Mqtt - disconnected({})",rc );
}

        fn on_connect(rc: i32) {
            if rc == 0 {
                println!(">> Mqtt - connected");
            } else {
                println!(">> Mqtt - failed to connect: ({})", rc);
            }
        }

        fn on_publish(mid: i32) {
            println!(">> Mqtt - Message ({}) published ", mid);
        }

        fn send(topic: &str, message: &str) -> bool {
            // Send - depending on QoS, mosquitto lib managed re-submission this the thread
            //
            // * NULL : Message Id (int *) this allow to latter get status of each message
            // * topic : topic to be used
            // * length of the message
            // * message
            // * qos (0,1,2)
            // * retain (boolean) - indicates if message is retained on broker or not
            // Should return MOSQ_ERR_SUCCESS
            let ret = publish(NULL, topic, message,len(), message, 1, true);
             (ret == MOSQ_ERR_SUCCESS)
        } // bool set_will(const char * _topic, const char * _message);
    }
}
