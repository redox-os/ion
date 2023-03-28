#!/usr/bin/env bash

RELEASE='target/release/ion'

if [[ ! -f "$RELEASE" ]]; then
  echo "$RELEASE does not exit. Please run (cargo build --release) before"
  exit 1
fi

if [[ ! -d "${DESTDIR}" ]]; then
  echo "Target folder ${DESTDIR} where the ion shell executable is to be installed, does not exits. Please ensure the folder exits" 
  exit 1
fi

install -Dm0755 "$RELEASE" "${DESTDIR}/ion"
