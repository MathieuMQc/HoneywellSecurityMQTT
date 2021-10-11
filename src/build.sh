#!/bin/sh
g++  -o honeywell -fdiagnostics-color --std=c++11 mqtt.cpp digitalDecoder.cpp analogDecoder.cpp main.cpp -lrtlsdr -lmosquittopp
