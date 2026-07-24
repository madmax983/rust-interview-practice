#!/bin/bash
sed -i 's/let _spawner_clone = spawner.clone();/let _spawner_clone = spawner;/g' src/systems/async_runtime.rs
