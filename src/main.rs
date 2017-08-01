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
#[macro_use] extern crate lazy_static;
extern crate pid_control;

use std::fs::File;
use std::io::prelude::*;
use std::num::ParseIntError;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use clap::{App, Arg};
use pid_control::{Controller, PIDController};

// You almost certainly need to change these parameters!
const APPLE_SMC: &str = "applesmc.768";
const TEMPERATURE: &str = "temp6";
const FANS: [&str; 2] = ["fan1", "fan2"];
const POWER_SUPPLY: &str = "ADP1";
const AC_ADJUSTMENT: f64 = 6.0;
const REPORT_INTERVAL: i32 = 12;

lazy_static! {
    static ref SMC_SYSFS_DIRECTORY: PathBuf = {
        Path::new("/sys/devices/platform").join(APPLE_SMC)
    };
    static ref POWER_SUPPLY_SYSFS_DIRECTORY: &'static Path = {
        Path::new("/sys/class/power_supply")
    };
}

struct Fan {
    identifier: String,
    cur_speed: i32,
    min_speed: i32,
    max_speed: i32
}

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

    let target_temperature: f64 = matches.value_of("target").unwrap_or("41.0").parse().unwrap();
    println!("base target temperature: {}", target_temperature);

    let mut fans: Vec<Fan> = FANS.into_iter().map(|id| {
        let mut file = File::create(SMC_SYSFS_DIRECTORY.join(format!("{}_manual", id))).unwrap();
        let one = "1\n".to_string();
        if let Err(e) = file.write_all(&one.into_bytes()) {
            panic!("Failed to set fan {} to manual control: {}", id, e);
        }
        Fan { identifier: id.to_string(),
              cur_speed: 0,
              min_speed: i32_from_smc_file(&format!("{}_min", id)).unwrap(),
              max_speed: i32_from_smc_file(&format!("{}_max", id)).unwrap() }
    }).collect();

    let p = -60.0;
    let i = -60.0;
    let d = -60.0;
    let mut controller = PIDController::new(p, i, d);
    controller.set_limits(fans.iter().map(|f| f.min_speed).min().unwrap() as f64,
                          fans.iter().map(|f| f.max_speed).max().unwrap() as f64);
    controller.set_target(target_temperature);

    let mut last_instant = Instant::now();
    let mut iterations = 0;
    let mut power_supply_online = false;
    loop {
        let power_supply_online_now = read_file(POWER_SUPPLY_SYSFS_DIRECTORY.join(POWER_SUPPLY).join("online")).trim().parse::<i32>().unwrap() == 1;
        if power_supply_online != power_supply_online_now {
            if power_supply_online && !power_supply_online_now {
                println!("setting baseline target temperature due to being on battery");
                controller.set_target(target_temperature);
            }
            else if !power_supply_online && power_supply_online_now {
                println!("adjusting target temperature up by {} degrees due to being on AC", AC_ADJUSTMENT);
                controller.set_target(target_temperature + AC_ADJUSTMENT);
            }
            power_supply_online = power_supply_online_now;
        }
        let current_temperature = i32_from_smc_file(&format!("{}_input", TEMPERATURE)).unwrap() as f64 / 1000.0;
        let new_fan_speed = {
            let duration = last_instant.elapsed();
            let delta = duration.as_secs() as f64 + duration.subsec_nanos() as f64 * 1e-9;
            last_instant = Instant::now();
            controller.update(current_temperature, delta).round() as i32
        };
        if iterations % REPORT_INTERVAL == 0 {
            println!("current temperature: {}, target temperature: {}, fan speed: {}", current_temperature, controller.target(), new_fan_speed);
        }
        for fan in &mut fans {
            let speed = clamp(new_fan_speed, fan.min_speed, fan.max_speed);
            if speed != fan.cur_speed {
                let mut file = File::create(SMC_SYSFS_DIRECTORY.join(format!("{}_output", fan.identifier))).unwrap();
                if let Err(e) = file.write_all(&format!("{}\n", speed).into_bytes()) {
                    println!("Failed to set fan speed: {}", e);
                }
                fan.cur_speed = speed;
            }
        }
        thread::sleep(Duration::from_secs(5));
        iterations += 1;
    }
}

fn clamp(value: i32, min: i32, max: i32) -> i32 {
    if value < min { min } else if value > max { max } else { value }
}

fn read_file(filename: PathBuf) -> String {
    let mut file = File::open(filename).unwrap();
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();
    content
}

fn i32_from_smc_file(filename: &str) -> Result<i32, ParseIntError> {
    read_file(SMC_SYSFS_DIRECTORY.join(filename)).trim().parse()
}
