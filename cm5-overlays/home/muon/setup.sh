#!/bin/sh

systemctl --user daemon-reload
systemctl --user enable audio-service.service
systemctl --user enable runtime-native.service
systemctl --user restart audio-service.service
systemctl --user restart runtime-native.service
