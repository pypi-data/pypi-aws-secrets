#!/usr/bin/env bash

case "$2" in
  *.gem)
    # Gemfiles are tar files with several individual gzip files inside.
    unar -D -k skip -q -o "$1" "$2"
    unar -D -k skip -q -o "$1" "$1"/data.tar.gz
    rm -rf "$1"/*.gz
    ;;
  *.tar)
    unar -D -k skip -q -o "$1" "$2"
    unar -D -k skip -q -o "$1" "$1"/contents.tar.gz
    rm -rf "$1"/*.gz
    ;;
  *)
    exec unar -D -k skip -q -o "$1" "$2"
    ;;
esac
