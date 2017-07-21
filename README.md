# macbookfan
A simple MacBook fan PID controller

My MacBook Pro fans don't run at all by default in Linux, leading to thermal throttling, burned thighs and shorter component lifespan.

There are some existing projects to deal with this, but I found they have confusing settings and still lead to my MacBook running uncomfortably hot to use as a laptop.

macbookfan has just one knob: a target temperature. A PID controller is used to modulate the fans to achieve that target.

This is very much a work in progress, but I already use it on my MacBook Pro, using a target temperature of `41`. This is probably much lower than most people will want to use, but I prefer a cold, noisy laptop to a warm, quiet laptop.

## Usage

`macbookfan -t 41`
