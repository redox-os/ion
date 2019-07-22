// This is based on the sprite example from piston-examples, available at https://github.com/PistonDevelopers/piston-examples

use ai_behavior::{Action, Sequence, Wait, WaitForever, While};
use ion_shell::{
    builtins::{BuiltinFunction, Status},
    types, Shell,
};
use piston_window::*;
use sprite::*;
use std::{cell::RefCell, fs::File, path::Path, rc::Rc};

fn main() {
    // This is specific to piston. It does not matter
    // Skip to the next *****
    let (width, height) = (300, 300);
    let opengl = OpenGL::V3_2;
    let mut window: PistonWindow = WindowSettings::new("piston: sprite", (width, height))
        .exit_on_esc(true)
        .graphics_api(opengl)
        .build()
        .unwrap();

    let id;
    let scene = RefCell::new(Scene::new());
    let mut texture_context = TextureContext {
        factory: window.factory.clone(),
        encoder: window.factory.create_command_buffer().into(),
    };
    let tex = Rc::new(
        Texture::from_path(
            &mut texture_context,
            Path::new("./examples/rust.png"),
            Flip::None,
            &TextureSettings::new(),
        )
        .expect("This example is meant to be run in the crate's root"),
    );
    let mut sprite = Sprite::from_texture(tex.clone());
    sprite.set_position(width as f64 / 2.0, height as f64 / 2.0);

    id = scene.borrow_mut().add_child(sprite);

    // Run a sequence of animations.
    let seq = RefCell::new(Sequence(vec![
        Action(Ease(EaseFunction::CubicOut, Box::new(ScaleTo(2.0, 0.5, 0.5)))),
        Action(Ease(EaseFunction::BounceOut, Box::new(MoveBy(1.0, 0.0, 100.0)))),
        Action(Ease(EaseFunction::ElasticOut, Box::new(MoveBy(2.0, 0.0, -100.0)))),
        Action(Ease(EaseFunction::BackInOut, Box::new(MoveBy(1.0, 0.0, -100.0)))),
        Wait(0.5),
        Action(Ease(EaseFunction::ExponentialInOut, Box::new(MoveBy(1.0, 0.0, 100.0)))),
        Action(Blink(1.0, 5)),
        While(
            Box::new(WaitForever),
            vec![
                Action(Ease(EaseFunction::QuadraticIn, Box::new(FadeOut(1.0)))),
                Action(Ease(EaseFunction::QuadraticOut, Box::new(FadeIn(1.0)))),
            ],
        ),
    ]));
    scene.borrow_mut().run(id, &seq.borrow());

    let rotate =
        RefCell::new(Action(Ease(EaseFunction::ExponentialInOut, Box::new(RotateTo(2.0, 360.0)))));
    scene.borrow_mut().run(id, &rotate.borrow());

    // *****

    // Because the shell and your application generally live side by side, you won't be able to
    // create builtins satisfying at compile-time the rust borrowing rules
    //
    // A `RefCell` is generally the solution
    let colors = RefCell::new([1.0, 1.0, 1.0, 1.0]);

    // Create a custom builtin.
    // Builtins provide means by which the user configuration can notify your application of a
    // change. You must provide a help description along with each one of them
    let toggle_animation_builtin: BuiltinFunction = &|_args, _shell| {
        let mut scene = scene.borrow_mut();
        scene.toggle(id, &seq.borrow());
        scene.toggle(id, &rotate.borrow());

        // The `Status` struct is an helper to avoid dealing with return codes and error messages
        // directly.
        //
        // Rather than printing to stdout and then to return 2, you can now leave the job to
        // ion and call Status::bad_argument(<error message>). Where possible, builtins should use
        // the helper
        Status::SUCCESS
    };

    // Another builtin
    let set_background_builtin: BuiltinFunction = &|args, _shell| {
        let inner = |colors: &[types::Str]| -> Result<_, std::num::ParseFloatError> {
            let red = colors[0].parse::<f32>()?;
            let green = colors[1].parse::<f32>()?;
            let blue = colors[2].parse::<f32>()?;
            let alpha = match colors.get(3) {
                Some(alpha) => Some(alpha.parse::<f32>()?),
                None => None,
            };
            Ok((red, green, blue, alpha))
        };
        if args.len() > 5 || args.len() < 4 {
            return Status::bad_argument(
                "Wrong number of arguments provided: please provide 3 or 4",
            );
        }
        match inner(&args[1..]) {
            Err(why) => Status::error(format!("Could not parse the input color: {}", why)),
            Ok((red, green, blue, alpha)) => {
                let colors = &mut colors.borrow_mut();
                colors[0] = red.max(0.).min(1.);
                colors[1] = green.max(0.).min(1.);
                colors[2] = blue.max(0.).min(1.);
                if let Some(alpha) = alpha {
                    colors[3] = alpha.max(0.).min(1.);
                }
                Status::SUCCESS
            }
        }
    };

    // Create a shell with default configuration as well as recommended builtins
    let mut shell = Shell::default();

    // Register the builtins, along with a short help text available with the `help` builtin
    shell.builtins_mut().add("toggle", toggle_animation_builtin, "toggle the animation");
    shell.builtins_mut().add("set_background", set_background_builtin, "set the background color");

    // Add global variables
    //
    // Variables should generally provided along with callbacks to allow to user to react to
    // changes. We'll leave it out in this example for the sake of simplicity
    let size = window.size();
    shell.variables_mut().set("WINDOW_WIDTH", size.width.to_string());
    shell.variables_mut().set("WINDOW_HEIGHT", size.height.to_string());

    // Load the config file. This is where a user can register callbacks, prepare itself, and setup
    // your application. All builtins and variables should be registered at this point
    //
    // Check for a global and a per-user config (here it lives in the same git repo, but in a real
    // application it should probably go to the ~/.config folder, or the recommended location on
    // your OS)
    if let Ok(file) =
        File::open("window-config.ion").or_else(|_| File::open("./examples/window-config.ion"))
    {
        if let Err(why) = shell.execute_command(file) {
            eprintln!("window: error in config file: {}", why);
        }
    }

    // Your application execution
    while let Some(e) = window.next() {
        scene.borrow_mut().event(&e);

        window.draw_2d(&e, |c, g, _| {
            clear(colors.clone().into_inner(), g);
            scene.borrow_mut().draw(c.transform, g);
        });

        if e.render_args().is_some() {
            // Get the on_render function
            if let Some(function) = shell.get_func("on_render") {
                // and then call it. N.B.: the first argument must always be ion
                if let Err(why) = shell.execute_function(&function, &["ion"]) {
                    // check for errors
                    eprintln!("window example: error in on_render callback: {}", why);
                }
            }
        }

        if let Some(Button::Keyboard(key)) = e.press_args() {
            if let Some(function) = shell.get_func("on_key") {
                if let Err(why) =
                    // provide a parameter for the callback
                    shell.execute_function(&function, &["ion", &key.code().to_string()])
                {
                    eprintln!("window example: error in on_key callback: {}", why);
                }
            }
        }

        if let Some([x, y]) = e.mouse_cursor_args() {
            if let Some(function) = shell.get_func("on_mouse") {
                if let Err(why) =
                    shell.execute_function(&function, &["ion", &x.to_string(), &y.to_string()])
                {
                    eprintln!("window example: error in on_mouse callback: {}", why);
                }
            }
        }
    }
}
