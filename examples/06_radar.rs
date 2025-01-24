//! Radar Game
//!
//! The aim of this game is to shoot vehicles that appear on the radar.
//! However, it is necessary to pay attention to the distinction between allie and enemy.
//! (Even if UFO)
//! Hitting allied forces is minus points.
//! In general, the basic functionalities of ggez are covered.
//! For example, drawing a line or a circle.
//! On the other hand, the subject of changing time-based geometric objects is also discussed.
//! In addition, the mouse click event was also evaluated and the player score was calculated accordingly.

use ggez::conf::WindowMode;
use ggez::event::MouseButton;
use ggez::graphics::Text;
use ggez::{
    event,
    glam::*,
    graphics::{self, Color},
    Context, GameError, GameResult,
};
use rand::Rng;
use std::f32::consts::PI;

const SCREEN_WIDTH: f32 = 400.;
const SCREEN_HEIGHT: f32 = 400.;
const ALLIE_HIT_POINT: i16 = -15;
const ENEMY_HIT_POINT: i16 = 5;

pub fn main() -> GameResult {
    let cb = ggez::ContextBuilder::new("Radar Jam", "BSŞ").window_mode(
        WindowMode::default()
            .dimensions(SCREEN_WIDTH, SCREEN_HEIGHT)
            .resizable(false),
    );
    let (mut ctx, event_loop) = cb.build()?;
    let state = GameState::new(&mut ctx)?;
    event::run(ctx, event_loop, state)
}

struct GameState {
    circle_r: f32,
    angle: f32,
    player: Player,
    vehicle: Vehicle,
}

struct Vehicle {
    pos: Vec2,
    is_alive: bool,
    is_spawned: bool,
    v_type: VehicleType,
    r: f32,
}

#[derive(Copy, Clone)]
enum VehicleType {
    Allie,
    Enemy,
    Unknown,
}

impl From<VehicleType> for Color {
    fn from(value: VehicleType) -> Self {
        match value {
            VehicleType::Allie => Color::from_rgb(141, 202, 255),
            VehicleType::Enemy => Color::from_rgb(134, 1, 17),
            VehicleType::Unknown => Color::from_rgb(60, 208, 112),
        }
    }
}

impl Default for Vehicle {
    fn default() -> Self {
        Self {
            pos: Vec2::new(0., 0.),
            is_spawned: false,
            is_alive: true,
            v_type: VehicleType::Unknown,
            r: 10.,
        }
    }
}

struct Player {
    click_point: Vec2,
    score: i16,
}

impl GameState {
    fn new(_ctx: &mut Context) -> GameResult<GameState> {
        Ok(GameState {
            circle_r: 0.,
            angle: 0.,
            player: Player {
                score: 0,
                click_point: Vec2::default(),
            },
            vehicle: Vehicle::default(),
        })
    }
}

impl event::EventHandler<GameError> for GameState {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        // Increase radius of radar circle
        if self.circle_r >= SCREEN_WIDTH / 2. {
            self.circle_r = 0.;
        }
        self.circle_r = &self.circle_r + 0.8;
        self.angle += PI / 90.;
        // Calculate random vehicle points
        if ctx.time.ticks() % 180 == 0 {
            let mut rng = rand::thread_rng();
            let angle: f32 = rng.gen_range(-2. * PI..2. * PI);
            self.vehicle.pos = Vec2::new(
                SCREEN_WIDTH * 0.5 + self.circle_r * angle.cos(),
                SCREEN_HEIGHT * 0.5 + self.circle_r * angle.sin(),
            );
            self.vehicle.is_spawned = true;
            let random_type: u8 = rng.gen_range(0..3);
            match random_type {
                0 => self.vehicle.v_type = VehicleType::Allie,
                1 => self.vehicle.v_type = VehicleType::Enemy,
                _ => self.vehicle.v_type = VehicleType::Unknown,
            }
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let mut canvas = graphics::Canvas::from_frame(ctx, Color::BLACK);

        // Radar circle draw
        let circle = graphics::Mesh::new_circle(
            ctx,
            graphics::DrawMode::stroke(2.),
            vec2(SCREEN_WIDTH / 2., SCREEN_HEIGHT / 2.),
            self.circle_r,
            0.5,
            Color::from_rgb(0, 143, 17),
        )?;
        canvas.draw(&circle, Vec2::new(0., 0.));

        // draw circle line
        let circle_line = graphics::Mesh::new_line(
            ctx,
            &[
                Vec2::new(SCREEN_WIDTH / 2., SCREEN_HEIGHT / 2.),
                Vec2::new(
                    SCREEN_WIDTH / 2. + self.circle_r * self.angle.cos(),
                    SCREEN_HEIGHT / 2. + self.circle_r * self.angle.sin(),
                ),
            ],
            2.,
            Color::from_rgb(0, 143, 17),
        )?;
        canvas.draw(&circle_line, Vec2::new(0., 0.));

        // Grid lines draw
        for i in 1..4 {
            let line = graphics::Mesh::new_line(
                ctx,
                &[
                    Vec2::new(0., i as f32 * 100.),
                    Vec2::new(SCREEN_WIDTH, i as f32 * 100.),
                ],
                2.,
                Color::from_rgb(123, 122, 121),
            )?;
            canvas.draw(&line, graphics::DrawParam::from([0., 0.]));

            let line = graphics::Mesh::new_line(
                ctx,
                &[
                    Vec2::new(i as f32 * 100., 0.),
                    Vec2::new(i as f32 * 100., SCREEN_HEIGHT),
                ],
                2.,
                Color::from_rgb(123, 122, 121),
            )?;
            canvas.draw(&line, graphics::DrawParam::from([0., 0.]));
        }

        // Vehicle draw
        if self.vehicle.is_spawned && self.vehicle.is_alive {
            let vehicle = graphics::Mesh::new_circle(
                ctx,
                graphics::DrawMode::fill(),
                self.vehicle.pos,
                self.vehicle.r,
                0.025,
                Color::from(self.vehicle.v_type),
            )?;
            canvas.draw(&vehicle, graphics::DrawParam::from([0., 0.]));
        }

        // Informal text draw
        let score_text = Text::new(format!(
            "Score:{}|r:{}|({})",
            self.player.score, self.circle_r, self.player.click_point
        ));
        canvas.draw(&score_text, graphics::DrawParam::from([0., 0.]));

        // Mouse cursor draw
        let mouse_line = graphics::Mesh::new_line(
            ctx,
            &[Vec2::new(0., -15.), Vec2::new(0., 15.)],
            2.,
            Color::from_rgb(165, 82, 76),
        )?;
        canvas.draw(&mouse_line, graphics::DrawParam::from(ctx.mouse.position()));
        let mouse_line = graphics::Mesh::new_line(
            ctx,
            &[Vec2::new(-15., 0.), Vec2::new(15., 0.)],
            2.,
            Color::from_rgb(165, 82, 76),
        )?;
        canvas.draw(&mouse_line, graphics::DrawParam::from(ctx.mouse.position()));

        canvas.finish(ctx)?;

        Ok(())
    }

    fn mouse_button_down_event(
        &mut self,
        _ctx: &mut Context,
        _button: MouseButton,
        x: f32,
        y: f32,
    ) -> Result<(), GameError> {
        self.player.click_point = Vec2::new(x, y);

        let distance = ((x - self.vehicle.pos.x).powi(2) + (y - self.vehicle.pos.y).powi(2)).sqrt();
        if distance <= 10. {
            // mouse click in circle
            match self.vehicle.v_type {
                VehicleType::Allie => self.player.score += ALLIE_HIT_POINT,
                VehicleType::Enemy => self.player.score += ENEMY_HIT_POINT,
                VehicleType::Unknown => {}
            }
            self.vehicle = Vehicle::default();
        }

        Ok(())
    }
}
