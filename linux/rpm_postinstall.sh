#!/bin/sh

systemctl daemon-reload

if [ "$1" -eq 1 ]; then
    systemctl preset obscuravpn.service
    systemctl start obscuravpn.service
fi

if [ "$1" -eq 2 ]; then
    systemctl try-restart obscuravpn.service
fi
