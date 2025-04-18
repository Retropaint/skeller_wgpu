//! Receives inputs from winit events. Most of actual input logic is handled per-module.

use crate::*;

use winit::event::ElementState;
use winit::keyboard::*;

pub fn keyboard_input(
    key: &winit::keyboard::KeyCode,
    state: &winit::event::ElementState,
    shared: &mut crate::shared::Shared,
) {
    if *key == KeyCode::KeyW {
        shared.armature.bones[1].tex_idx = 0;
        shared.armature.bones[2].tex_idx = 0;
    }

    // record all pressed keys (and remove released ones)
    if *state == ElementState::Pressed {
        let mut add = true;
        for pressed_key in &mut shared.input.pressed {
            if key == pressed_key {
                add = false;
                break;
            }
        }
        if add {
            shared.input.pressed.push(*key);
        }
    } else {
        for (i, pressed_key) in &mut shared.input.pressed.iter().enumerate() {
            if pressed_key == key {
                shared.input.pressed.remove(i);
                break;
            }
        }
    }

    if is_pressing(KeyCode::Equal, &shared) {
        ui::set_zoom(shared.camera.zoom - 0.1, shared)
    } else if is_pressing(KeyCode::Minus, &shared) {
        ui::set_zoom(shared.camera.zoom + 0.1, shared);
    }

    if *key == KeyCode::SuperLeft {
        if *state == ElementState::Pressed {
            shared.input.modifier = 1;
        } else {
            shared.input.modifier = -1;
        }
    }

    let mut undo = false;
    let mut redo = false;
    if shared.input.modifier == 1 {
        if is_pressing(KeyCode::KeyZ, &shared) && shared.undo_actions.len() != 0 {
            undo = true;
        } else if is_pressing(KeyCode::KeyY, &shared) && shared.redo_actions.len() != 0 {
            redo = true;
        }
    }

    if undo || redo {
        let action: Action;
        if undo {
            action = shared.undo_actions.last().unwrap().clone();
        } else {
            action = shared.redo_actions.last().unwrap().clone();
        }
        let mut new_action = action.clone();

        match &action.action {
            ActionEnum::Bone => {
                if action.action_type == ActionType::Created {
                    shared.selected_bone_idx = usize::MAX;
                    if undo {
                        shared.armature.bones.pop();
                    } else {
                        armature_window::new_bone(&mut shared.armature.bones);
                    }
                } else {
                    new_action.bone = shared.armature.bones[action.id as usize].clone();
                    shared.armature.bones[action.id as usize] = action.bone.clone();
                }
            }
            ActionEnum::Animation => {
                if action.action_type == ActionType::Created {
                    shared.ui.anim.selected = usize::MAX;
                    if undo {
                        shared.armature.animations.pop();
                    } else {
                        keyframe_editor::new_animation(shared);
                    }
                } else {
                    new_action.animation = shared.armature.animations[action.id as usize].clone();
                    shared.armature.animations[action.id as usize] = action.animation.clone();
                }
            }
            _ => {}
        }

        if undo {
            shared.redo_actions.push(new_action);
            shared.undo_actions.pop();
        } else {
            shared.undo_actions.push(new_action);
            shared.redo_actions.pop();
        }
    }
}

pub fn mouse_input(
    button: &crate::MouseButton,
    state: &ElementState,
    shared: &mut crate::shared::Shared,
) {
    // mouse inputs coming from winit only do so if it's not on egui
    if *button == MouseButton::Left {
        if *state == ElementState::Pressed {
            shared.input.on_ui = false;
        } else {
            shared.input.on_ui = true;
        }
    }

    // increase mouse_left if it's being held down
    if shared.input.mouse_left >= 0 {
        shared.input.mouse_left += 1;
    }
}

pub fn mouse_wheel_input(delta: MouseScrollDelta, shared: &mut Shared) {
    let sens_reducer = 100.;
    match delta {
        MouseScrollDelta::LineDelta(_x, y) => {
            ui::set_zoom(shared.camera.zoom + (y as f32 / sens_reducer), shared);
        }
        MouseScrollDelta::PixelDelta(pos) => {
            ui::set_zoom(shared.camera.zoom + (pos.y as f32 / sens_reducer), shared);
        }
    }
}

pub fn is_pressing(key: KeyCode, shared: &Shared) -> bool {
    for k in &shared.input.pressed {
        if *k == key {
            return true;
        }
    }
    false
}
