#!/bin/sh
RUSTUP=0 make # By default RUSTUP equals 1, which is for developmental purposes
sudo make install prefix=/usr
sudo make update-shells prefix=/usr
