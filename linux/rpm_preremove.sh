#!/bin/sh

if [ "$1" -eq 0 ]; then
    systemctl --no-reload disable --now obscuravpn.service
fi
