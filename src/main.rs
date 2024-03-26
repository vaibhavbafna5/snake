#![no_main]
#![no_std]

use cortex_m_rt::entry;
use rtt_target::{rtt_init_print, rprintln};
use heapless::Vec;
use panic_rtt_target as _;
use microbit::{
    board::Board,
    display::blocking::Display,
    hal::{prelude::*, Timer},
};


#[cfg(feature = "v2")]
use microbit::{
    hal::twim,
    pac::twim0::frequency::FREQUENCY_A,
};

#[cfg(feature = "v2")]
use microbit::{
    hal::prelude::*,
    hal::uarte,
    hal::uarte::{Baudrate, Parity},
};

#[cfg(feature = "v2")]
mod serial_setup;
#[cfg(feature = "v2")]
use serial_setup::UartePort;

use lsm303agr::{
    AccelOutputDataRate, Lsm303agr, MagOutputDataRate
};

struct LcgRng {
    // pseudorandom number generator
    state: u32,
}

impl LcgRng {
    fn new(seed: u32) -> Self {
        // seed is generated from accelerometer data, see below for more
        LcgRng { state: seed }
    }

    fn next(&mut self) -> u8 {
        // generates the next pseudorandom number
        const MULTIPLIER: u32 = 1664525;
        const INCREMENT: u32 = 1013904223;
        self.state = self.state.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT);
        self.state as u8
    }

    fn next_in_range(&mut self, min: u8, max: u8) -> u8 {
        // takes the random number and puts it in bounds
        (self.next() % (max - min + 1)) + min
    }
}


pub struct Jungle {
    // captures all the relevant parts of the game
    snake: Snake,  // fairly obvious, represents snake
    basemap: [[u8; 5]; 5],  // represents the underlying grid that will be displayed
    previous_direction: char, 
    nugget: (u8, u8),  // snake's target
    rng: LcgRng,  // pseudorandom number generator
}

impl Jungle {
    pub fn new(snake: Snake, nugget: (u8, u8), rng: LcgRng) -> Self {
        // initializes the jungle
        Self {
            basemap: [
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0],
            ],
            snake: snake,
            previous_direction: 'R',
            nugget: nugget,
            rng: rng,
        }
    }

    pub fn update(&mut self, new_direction: Option<char>) {      
        /*
        Main driver of the game.
        - iterate over each segment and update it
        - check to see if the nugget was eaten
        - if the nugget was eaten, append the segment correctly & generate a new one
        - change direction of the snake if this was indicated
        */
        let optional_head = self.snake.segments.get(0).cloned();
        let mut _new_direction : char;
        self.basemap[self.nugget.0 as usize][self.nugget.1 as usize] = 1;

        match optional_head {
            Some(head) => {

                let mut current_segment_index = 0;
                let mut last_segment_clone = self.snake.segments.last().unwrap().clone();

                while current_segment_index < self.snake.segments.len() {
                    let current_segment = &mut self.snake.segments[current_segment_index];
                    let current_segment_x = current_segment.point.0;
                    let current_segment_y = current_segment.point.1;

                    match new_direction {
                        Some(_new_direction) => {
                            if _new_direction != self.previous_direction && (
                                // stupidity since sets are seemingly unusable?
                                _new_direction == 'R' ||
                                _new_direction == 'L' ||
                                _new_direction == 'U' ||
                                _new_direction == 'D'
                            ) {
                                current_segment.add_checkpoint(head.point.0, head.point.1, _new_direction);
                            }
                        },
                        None => (),
                    }

                    // call update on the segment
                    current_segment.update();

                    // if the segment has "eaten" the nugget, update snake accordingly 
                    if current_segment.point.0 == self.nugget.0 as i8 && current_segment.point.1 == self.nugget.1 as i8 {
                        let mut segment = push_segment_to_back(&last_segment_clone, last_segment_clone.default_direction);
                        rprintln!("New segment: {}, {}, {}", segment.point.0, segment.point.1, segment.default_direction);
                        self.snake.add_segment(segment);

                        self.nugget.0 = self.rng.next_in_range(0, 4);
                        self.nugget.1 = self.rng.next_in_range(0, 4);
                        rprintln!("New nugget: {}, {}", self.nugget.0, self.nugget.1);
                    }

                    // TODO: death probably goes here!
                    self.basemap[current_segment_x as usize][current_segment_y as usize] = 1;                    
                    current_segment_index += 1;
                }
            },
            None => ()
        }

        // update direction based on input
        match new_direction {
            Some(_new_direction) => {
                if _new_direction != self.previous_direction {
                    self.previous_direction = _new_direction;
                }
            },
            None => ()
        }
    }
}
pub struct Snake {
    // represents snake, which is composed of "Segments"
    segments: Vec<Segment, 25>,
}

impl Snake {
    pub fn new() -> Self {
        // initialize the snake with the head at (1, 1)
        let mut body = Vec::new();
        body.push(Segment {
            point: (1,1),
            default_direction: 'R',
            checkpoints: Vec::new(),
        });
        body.push(Segment {
            point: (1, 0),
            default_direction: 'R',
            checkpoints: Vec::new(),
        });

        Snake {
            segments: body,
        }
    }

    pub fn add_segment(&mut self, segment: Segment) {
        // append new segment to the snake
        self.segments.push(segment);
    }
}


#[derive(Clone)]
pub struct Segment {
    /* 
    Segment is the discrete element that makes up a snake.
    - point indicates where the segment currently is
    - default direction indicates which way the segment should be moving
    - checkpoints is used to store the location of user-indicated turns
    
    Checkpoints are the secret sauce. This is how the snake "knows" when to
    turn after the user has entered a turn.
    */
    point: (i8, i8),
    default_direction: char,
    checkpoints: Vec<(i8, i8, char), 10>,
}

impl Segment {
    pub fn add_checkpoint(&mut self, x: i8, y: i8, direction: char) {
        // self explanatory, used to add a new checkpoint to the segment
        self.checkpoints.push((x, y, direction));
    }

    pub fn update(&mut self) {
        // update each segment based on checkpoints
        let current_checkpoint = self.checkpoints.get(0);
        match current_checkpoint {
            Some(value) => {
                if (self.point.0 == value.0 && self.point.1 == value.1) {
                    self.default_direction = value.2;
                    self.checkpoints.remove(0);
                }
            },
            None => ()
        }

        // update point's location based on direction
        if self.default_direction == 'R' {
            self.point.1 += 1;
            if self.point.1 == 5 {
                self.point.1 = 0;
            }
        }

        if self.default_direction == 'L' {
            self.point.1 -= 1;
            if self.point.1 == -1 {
                self.point.1 = 4;
            }
        }

        if self.default_direction == 'U' {
            self.point.0 -= 1;
            if self.point.0 == -1 {
                self.point.0 = 4;
            }
        }

        if self.default_direction == 'D' {
            self.point.0 += 1;
            if self.point.0 == 5 {
                self.point.0 = 0;
            }
        }
    }
}

pub fn push_segment_to_back(last_segment: &Segment, direction: char) -> Segment {
    // find the location for the segment that will be appended
    rprintln!("Incoming direction: {}", direction);
    let mut new_segment = Segment {
        point: (last_segment.point.0, last_segment.point.1),
        default_direction: direction,
        checkpoints: Vec::new(),
    };

    // copy checkpoints from the last segment
    new_segment.checkpoints.clone_from(&last_segment.checkpoints);

    // update segment based on direction, handle edge cases appropriately
    if direction == 'R' {
        new_segment.point.1 -= 1;
        if new_segment.point.1 == -1 {
            new_segment.point.1 = 4;
        }
    }

    if (direction == 'L') {
        new_segment.point.1 += 1;
        if new_segment.point.1 == 5 {
            new_segment.point.1 = 0
        }
    }

    if direction == 'D' {
        new_segment.point.0 -= 1;
        if new_segment.point.0 == -1 {
            new_segment.point.1 = 4;
        }
    }


    if direction == 'U' {
        new_segment.point.0 += 1;
        if new_segment.point.0 == 5 {
            new_segment.point.0 = 0;
        }
    }
    return new_segment;
}


#[entry]
fn main() -> ! {
    // initialize board elements
    rtt_init_print!();
    let board = microbit::Board::take().unwrap();
    let mut timer = Timer::new(board.TIMER0);
    let mut display = Display::new(board.display_pins);

    // initialize serial interface
    #[cfg(feature = "v2")]
    let mut serial = {
        let serial = uarte::Uarte::new(
            board.UARTE0,
            board.uart.into(),
            Parity::EXCLUDED,
            Baudrate::BAUD115200,
        );
        UartePort::new(serial)
    };

    #[cfg(feature = "v2")]
    let mut i2c = { twim::Twim::new(board.TWIM0, board.i2c_internal.into(), FREQUENCY_A::K100) };

    // initialization for accelerometer/magnet
    let mut sensor = Lsm303agr::new_with_i2c(i2c);
    sensor.init().unwrap();
 
    sensor.set_accel_odr(AccelOutputDataRate::Hz50).unwrap();
    sensor.set_mag_odr(MagOutputDataRate::Hz50).unwrap();

    let mut sensor = sensor.into_mag_continuous().ok().unwrap();

    // read sensor data to get seed
    let mut sensor_data = sensor.accel_data().unwrap();
    let mut seed = sensor_data.y as u32;

    // intialize randomizer
    let mut rng = LcgRng::new(seed);

    // randomly generate nugget coords
    let random_x: u8 = rng.next_in_range(0, 4);
    let random_y: u8 = rng.next_in_range(0, 4);
    rprintln!("Nugget x: {}", random_x);
    rprintln!("Nugget y: {}", random_y);

    // initialize snake in the jungle w/ a basemap & a nugget
    let mut nugget: (u8, u8) = (random_x, random_y);
    let mut snake = Snake::new();
    let mut jungle: Jungle = Jungle::new(snake, nugget, rng);
    let mut basemap: [[u8; 5]; 5] = [
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
    ];

    // set initial conditions
    let mut previous_snake_direction : char = 'R';

    loop {
        // read direction
        let serial_byte = serial.read();
        let mut snake_direction: Option<char> = None;

        match serial_byte {
            Ok(x) => {
                snake_direction = Some(x as char);
                rprintln!("Snake direction: {}", x as char);
            }
            Err(_) => {},
        }

        // render the snake in the jungle
        jungle.update(snake_direction);
        display.show(&mut timer, jungle.basemap, 175);

        // clear the basemap
        jungle.basemap = [
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
            [0, 0, 0, 0, 0],
        ];

        // delay for aesthetics
        timer.delay_ms(500_u32);

    }
}