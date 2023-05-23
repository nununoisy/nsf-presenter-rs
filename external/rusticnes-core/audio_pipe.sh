#!/bin/bash
rm audiodump.raw
touch audiodump.raw
tail -f audiodump.raw | play --type raw --encoding signed-integer --bits 16 --endian big --channels 1 --rate 44100 -
