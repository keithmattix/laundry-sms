extern crate rppal;
extern crate arrayvec;
extern crate twilio;
#[macro_use] extern crate log;
extern crate simplelog;

use std::{env, thread};
use std::fs::File;

use simplelog::*;

use std::time::{Duration};
use twilio::{Client, OutboundMessage}

use rppal::gpio::{Gpio, Mode, Level, Trigger};
use arrayvec::{ArrayVec};
// The GPIO module uses BCM pin numbering. BCM GPIO 17 is tied to physical pin 11.
const GPIO_VIBRATION: u8 = 17;
const FROM_NUMBER: &str = "+19015860851";
const TO_NUMBER: &str = "+19018257798";

fn main() {
  WriteLogger::init(LevelFilter::Info, Config::default(), File::create("laundry_sms.log").unwrap()).unwrap();
  let twilio_account_env = env::var("TWILIO_ACCOUNT_ID");
  let twilio_auth_env = env::var("TWILIO_AUTH_TOKEN");
  let account_id;
  let auth_token;
  match twilio_account_env {
    Ok(value) => account_id = value,
    Err(_err) => panic!("TWILIO_ACCOUNT_ID not set: Please make sure both twilio env vars are set."),
  }
  match twilio_auth_env {
    Ok(value) => auth_token = value,
    Err(_err) => panic!("TWILIO_AUTH_TOKEN not set: Please make sure both twilio env vars are set."),
  }
  let client = Client::new(account_id, auth_token);

  let mut gpio = Gpio::new().unwrap();
  gpio.set_mode(GPIO_VIBRATION, Mode::Input);

  match gpio.set_interrupt(GPIO_VIBRATION, Trigger::Both) {
    Ok(_) => (),
    Err(err) => error!("GPIO error occurred: {}.", err),
  }
  let mut signals = ArrayVec::<[Level; 3]>::new();
  let mut active: bool = false; // active represents whether or not the laundry machine is active
  loop {
    let level = gpio.poll_interrupt(GPIO_VIBRATION, false, None).unwrap().unwrap();
    match signals.try_insert(0, level) { // insert the new reading into the fixed vector
      Ok(_) => (),
      Err(_e) => {
        signals.remove(2);
        signals.insert(0, level);
      }
    }
    // if every element in the signals arravec is low, then the pi has been idle for 3 minutes
    let is_idle = signals.as_slice().iter().all(|&signal| signal == Level::Low);
    if level == Level::High {
      if !active && !is_idle {
        // If the machine was idle and now it hasn't been for 3 minutes, it's started
        active = true;
        info!("New load started!");
      }
    } else if level == Level::Low {
      if active && is_idle {
        // If the machine was active and now it's been idle for 3 minutes, it's done
        active = false;
        send_laundry_finished_message(client);
        info!("Load finished!");
      }
    }
    thread::sleep(Duration::from_secs(60));
  }
}

fn send_laundry_finished_message(client: Client) {
  let body = "Your laundry is ready for you to pick up!";
  match client.send_message(OutboundMessage::new(FROM_NUMBER, TO_NUMBER, body)) {
    Ok(_message) => info!("Sent notification to {}.", TO_NUMBER),
    Err(_e) => error!("Error delivering message to {}.", TO_NUMBER),
  }
}