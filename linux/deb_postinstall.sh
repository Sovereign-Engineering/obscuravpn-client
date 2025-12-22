#!/bin/sh

deb-systemd-helper enable obscuravpn.service
deb-systemd-invoke start obscuravpn.service
