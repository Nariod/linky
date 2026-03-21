#!/bin/bash

# Start the server from /app so that generate commands find links/ source dirs
cd /app
exec "$@"
