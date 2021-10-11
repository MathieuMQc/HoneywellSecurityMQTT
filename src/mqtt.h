#ifndef __MQTT_H__
#define __MQTT_H__

#include <stdint.h>
#include <mosquittopp.h>

class Mqtt : public mosqpp::mosquittopp
{
    private:
        const char      *host;
        const char      *id;
        int             port;
        int             keepalive;
        const char      *will_message;
        const char      *will_topic;

        void on_connect(int rc);
        void on_disconnect(int rc);
        void on_publish(int mid);

    public:
        Mqtt(const char *id, const char *host, int port, const char *username, const char *password, const char *will_topic, const char *will_message);
        ~Mqtt();
        bool send(const char * _topic, const char * _message);
        bool set_will(const char * _topic, const char * _message);
};

#endif
