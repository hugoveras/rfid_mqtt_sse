docker build -t rfid_mqtt_sse .



docker run --restart=always --publish 50054:50054 --name=rfid_mqtt_sse -d rfid_mqtt_sse

docker rm rfid_mqtt_sse -f
