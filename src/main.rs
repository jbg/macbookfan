/*
 * Copyright 2017 Jasper Bryant-Greene
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#[macro_use] extern crate clap;
extern crate pid_control;

use std::fs::File;
use std::io::prelude::*;
use std::thread;
use std::time::{Duration, Instant};

use clap::{App, Arg};
use pid_control::{Controller, PIDController, DerivativeMode};

// You almost certainly need to change these parameters!
const TEMPERATURE_SYSFS_FILE: &str = "/sys/devices/platform/applesmc.768/temp7_input";
const FAN1_SYSFS_FILE: &str = "/sys/devices/platform/applesmc.768/fan1_output";
const FAN1_MIN: f64 = 2160.0;
const FAN1_MAX: f64 = 6156.0;
const FAN2_SYSFS_FILE: &str = "/sys/devices/platform/applesmc.768/fan2_output";
const FAN2_MIN: f64 = 2000.0;
const FAN2_MAX: f64 = 5700.0;
const REPORT_INTERVAL: i32 = 12;

fn main() {
    let app = App::new("macbookfan").version(crate_version!())
                  .author("Jasper Bryant-Greene <jbg@rf.net.nz>")
                  .about("Controls the fan in your MacBook")
                  .arg(Arg::with_name("target")
                           .short("t")
                           .long("target")
                           .value_name("TEMPERATURE")
                           .help("Sets the target temperature for your MacBook CPU in degrees Celsius")
                           .takes_value(true));
    println!("{} {}", app.get_name(), crate_version!());
    let matches = app.get_matches();
    let target_temperature: f64 = matches.value_of("target").unwrap_or("35.0").parse().unwrap();
    println!("target temperature: {}", target_temperature);
    let p = -100.0;
    let i = -100.0;
    let d = -100.0;
    let mut controller = PIDController::new(p, i, d);
    controller.out_min = f64::min(FAN1_MIN, FAN2_MIN);
    controller.out_max = f64::max(FAN1_MAX, FAN2_MAX);
    controller.d_mode = DerivativeMode::OnError;
    controller.set_target(target_temperature);

    let mut last_instant = Instant::now();
    let mut iterations = 0;
    let mut cur_fan1_speed = 0;
    let mut cur_fan2_speed = 0;
    loop {
        let current_temperature = {
            let mut file = File::open(TEMPERATURE_SYSFS_FILE).unwrap();
            let mut content = String::new();
            file.read_to_string(&mut content).unwrap();
            let raw: f64 = content.trim().parse().unwrap();
            raw / 1000.0
        };
        let new_fan_speed = {
            let duration = last_instant.elapsed();
            let delta = duration.as_secs() as f64 + duration.subsec_nanos() as f64 * 1e-9;
            last_instant = Instant::now();
            controller.update(current_temperature, delta)
        };
        if iterations % REPORT_INTERVAL == 0 {
            println!("t={}, f={}", current_temperature, new_fan_speed.round());
        }
        {
            let fan1_speed = clamp(new_fan_speed, FAN1_MIN, FAN1_MAX);
            if fan1_speed != cur_fan1_speed {
                let mut file = File::create(FAN1_SYSFS_FILE).unwrap();
                if let Err(e) = file.write_all(&format!("{}\n", fan1_speed).into_bytes()) {
                    println!("Failed to set fan1 speed: {}", e);
                }
                cur_fan1_speed = fan1_speed;
            }
            let fan2_speed = clamp(new_fan_speed, FAN2_MIN, FAN2_MAX);
            if fan2_speed != cur_fan2_speed {
                let mut file = File::create(FAN2_SYSFS_FILE).unwrap();
                if let Err(e) = file.write_all(&format!("{}\n", fan2_speed).into_bytes()) {
                    println!("Failed to set fan2 speed: {}", e);
                }
                cur_fan2_speed = fan2_speed;
            }
        }
        thread::sleep(Duration::from_secs(5));
        iterations += 1;
    }
}

fn clamp(value: f64, min: f64, max: f64) -> i32 {
    (if value < min { min } else if value > max { max } else { value }).round() as i32
}
