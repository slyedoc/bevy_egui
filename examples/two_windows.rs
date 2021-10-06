use bevy::{
    prelude::*,
    render::{
        camera::{ActiveCameras, Camera},
        pass::*,
        render_graph::{
            base::MainPass, CameraNode, PassNode, RenderGraph, WindowSwapChainNode,
            WindowTextureNode,
        },
        texture::{Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsage},
    },
    window::{CreateWindow, WindowDescriptor, WindowId},
};
use bevy_egui::{egui, EguiContext, EguiPlugin};

const BEVY_TEXTURE_ID: u64 = 0;

/// This example creates a second window and draws a mesh from two different cameras.
fn main() {
    App::new()
        .insert_resource(Msaa { samples: 4 })
        .init_resource::<SharedUiState>()
        .add_state(AppState::CreateWindow)
        .add_plugins(DefaultPlugins)
        .add_plugin(EguiPlugin)
        .add_startup_system(load_assets.system())
        .add_system_set(
            SystemSet::on_update(AppState::CreateWindow).with_system(setup_window.system()),
        )
        .add_system_set(SystemSet::on_update(AppState::Setup).with_system(setup.system()))
        .add_system_set(SystemSet::on_update(AppState::Done).with_system(ui_second_window.system()))
        .add_system(ui_first_window.system())
        .run();
}

struct SecondWindow {
    id: WindowId,
}

// NOTE: this "state based" approach to multiple windows is a short term workaround.
// Future Bevy releases shouldn't require such a strict order of operations.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
enum AppState {
    CreateWindow,
    Setup,
    Done,
}

fn setup_window(
    mut app_state: ResMut<State<AppState>>,
    mut create_window_events: EventWriter<CreateWindow>,
) {
    let window_id = WindowId::new();

    // sends out a "CreateWindow" event, which will be received by the windowing backend
    create_window_events.send(CreateWindow {
        id: window_id,
        descriptor: WindowDescriptor {
            width: 800.,
            height: 600.,
            vsync: false,
            title: "second window".to_string(),
            ..Default::default()
        },
    });

    app_state.set(AppState::Setup).unwrap();
}

mod second_window {
    pub const SWAP_CHAIN: &str = "second_window_swap_chain";
    pub const DEPTH_TEXTURE: &str = "second_window_depth_texture";
    pub const CAMERA_NODE: &str = "secondary_camera";
    pub const CAMERA_NAME: &str = "Secondary";
    pub const SAMPLED_COLOR_ATTACHMENT: &str = "second_multi_sampled_color_attachment";
    pub const PASS: &str = "second_window_pass";
}

fn load_assets(mut egui_context: ResMut<EguiContext>, assets: Res<AssetServer>) {
    let texture_handle = assets.load("icon.png");
    egui_context.set_egui_texture(BEVY_TEXTURE_ID, texture_handle);
}

fn setup_pipeline(
    render_graph: &mut RenderGraph,
    active_cameras: &mut ActiveCameras,
    msaa: &Msaa,
    window_id: WindowId,
) {
    // here we setup our render graph to draw our second camera to the new window's swap chain

    // add a swapchain node for our new window
    render_graph.add_node(
        second_window::SWAP_CHAIN,
        WindowSwapChainNode::new(window_id),
    );

    // add a new depth texture node for our new window
    render_graph.add_node(
        second_window::DEPTH_TEXTURE,
        WindowTextureNode::new(
            window_id,
            TextureDescriptor {
                format: TextureFormat::Depth32Float,
                usage: TextureUsage::OUTPUT_ATTACHMENT,
                sample_count: msaa.samples,
                ..Default::default()
            },
        ),
    );

    // add a new camera node for our new window
    render_graph.add_system_node(
        second_window::CAMERA_NODE,
        CameraNode::new(second_window::CAMERA_NAME),
    );

    // add a new render pass for our new window / camera
    let mut second_window_pass = PassNode::<&MainPass>::new(PassDescriptor {
        color_attachments: vec![msaa.color_attachment(
            TextureAttachment::Input("color_attachment".to_string()),
            TextureAttachment::Input("color_resolve_target".to_string()),
            Operations {
                load: LoadOp::Clear(Color::rgb(0.5, 0.5, 0.8)),
                store: true,
            },
        )],
        depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
            attachment: TextureAttachment::Input("depth".to_string()),
            depth_ops: Some(Operations {
                load: LoadOp::Clear(1.0),
                store: true,
            }),
            stencil_ops: None,
        }),
        sample_count: msaa.samples,
    });

    second_window_pass.add_camera(second_window::CAMERA_NAME);
    active_cameras.add(second_window::CAMERA_NAME);

    render_graph.add_node(second_window::PASS, second_window_pass);

    render_graph
        .add_slot_edge(
            second_window::SWAP_CHAIN,
            WindowSwapChainNode::OUT_TEXTURE,
            second_window::PASS,
            if msaa.samples > 1 {
                "color_resolve_target"
            } else {
                "color_attachment"
            },
        )
        .unwrap();

    render_graph
        .add_slot_edge(
            second_window::DEPTH_TEXTURE,
            WindowTextureNode::OUT_TEXTURE,
            second_window::PASS,
            "depth",
        )
        .unwrap();

    render_graph
        .add_node_edge(second_window::CAMERA_NODE, second_window::PASS)
        .unwrap();

    if msaa.samples > 1 {
        render_graph.add_node(
            second_window::SAMPLED_COLOR_ATTACHMENT,
            WindowTextureNode::new(
                window_id,
                TextureDescriptor {
                    size: Extent3d {
                        depth_or_array_layers: 1,
                        width: 1,
                        height: 1,
                    },
                    mip_level_count: 1,
                    sample_count: msaa.samples,
                    dimension: TextureDimension::D2,
                    format: TextureFormat::default(),
                    usage: TextureUsage::OUTPUT_ATTACHMENT,
                },
            ),
        );

        render_graph
            .add_slot_edge(
                second_window::SAMPLED_COLOR_ATTACHMENT,
                WindowSwapChainNode::OUT_TEXTURE,
                second_window::PASS,
                "color_attachment",
            )
            .unwrap();
    }

    bevy_egui::setup_pipeline(
        render_graph,
        msaa,
        bevy_egui::RenderGraphConfig {
            window_id,
            egui_pass: "egui_pass2",
            main_pass: second_window::PASS,
            swap_chain_node: second_window::SWAP_CHAIN,
            depth_texture: second_window::DEPTH_TEXTURE,
            sampled_color_attachment: second_window::SAMPLED_COLOR_ATTACHMENT,
            transform_node: "egui_transform2",
        },
    );
}

fn setup(
    mut commands: Commands,
    mut app_state: ResMut<State<AppState>>,
    windows: Res<Windows>,
    mut active_cameras: ResMut<ActiveCameras>,
    mut render_graph: ResMut<RenderGraph>,
    mut meshes: ResMut<Assets<Mesh>>,
    msaa: Res<Msaa>,
) {
    // get the non-default window id
    let window_id = match windows
        .iter()
        .find(|w| w.id() != WindowId::default())
        .map(|w| w.id())
    {
        Some(x) => x,
        None => return,
    };

    setup_pipeline(&mut render_graph, &mut active_cameras, &msaa, window_id);

    // SETUP SCENE

    // add entities to the world
    commands.spawn_bundle(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Cube { size: 1.0 })),
        ..Default::default()
    });
    // light
    commands.spawn_bundle(PointLightBundle {
        transform: Transform::from_xyz(4.0, 5.0, 4.0),
        ..Default::default()
    });
    // main camera
    commands.spawn_bundle(PerspectiveCameraBundle {
        transform: Transform::from_xyz(0.0, 0.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    });
    // second window camera
    commands.spawn_bundle(PerspectiveCameraBundle {
        camera: Camera {
            name: Some("Secondary".to_string()),
            window: window_id,
            ..Default::default()
        },
        transform: Transform::from_xyz(6.0, 0.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    });

    commands.insert_resource(SecondWindow { id: window_id });

    app_state.set(AppState::Done).unwrap();
}

#[derive(Default)]
struct UiState {
    input: String,
}

#[derive(Default)]
struct SharedUiState {
    shared_input: String,
}

fn ui_first_window(
    egui_context: Res<EguiContext>,
    mut ui_state: Local<UiState>,
    mut shared_ui_state: ResMut<SharedUiState>,
) {
    egui::Window::new("First Window")
        .scroll(true)
        .show(egui_context.ctx(), |ui| {
            ui.horizontal(|ui| {
                ui.label("Write something: ");
                ui.text_edit_singleline(&mut ui_state.input);
            });
            ui.horizontal(|ui| {
                ui.label("Shared input: ");
                ui.text_edit_singleline(&mut shared_ui_state.shared_input);
            });

            ui.add(egui::widgets::Image::new(
                egui::TextureId::User(BEVY_TEXTURE_ID),
                [256.0, 256.0],
            ));
        });
}

fn ui_second_window(
    egui_context: Res<EguiContext>,
    second_window: Res<SecondWindow>,
    mut ui_state: Local<UiState>,
    mut shared_ui_state: ResMut<SharedUiState>,
) {
    egui::Window::new("Second Window").scroll(true).show(
        egui_context.ctx_for_window(second_window.id),
        |ui| {
            ui.horizontal(|ui| {
                ui.label("Write something else: ");
                ui.text_edit_singleline(&mut ui_state.input);
            });
            ui.horizontal(|ui| {
                ui.label("Shared input: ");
                ui.text_edit_singleline(&mut shared_ui_state.shared_input);
            });

            ui.add(egui::widgets::Image::new(
                egui::TextureId::User(BEVY_TEXTURE_ID),
                [256.0, 256.0],
            ));
        },
    );
}
