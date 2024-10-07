// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::include_modules!();

use anyhow::{bail, Result};

use gst::prelude::*;

use crate::ui::WindowUpdater;

fn try_gstreamer_video_frame_to_pixel_buffer(
    frame: &gst_video::VideoFrame<gst_video::video_frame::Readable>,
) -> Result<slint::SharedPixelBuffer<slint::Rgb8Pixel>> {
    match frame.format() {
        gst_video::VideoFormat::Rgb => {
            let mut slint_pixel_buffer =
                slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(frame.width(), frame.height());
            frame
                .buffer()
                .copy_to_slice(0, slint_pixel_buffer.make_mut_bytes())
                .expect("Unable to copy to slice!"); // Copies!
            Ok(slint_pixel_buffer)
        },
        _ => {
            bail!(
                "Cannot convert frame to a slint RGB frame because it is format {}",
                frame.format().to_str()
            )
        },
    }
}

pub fn init_pipeline(video_uri: &String, width: u32, max_rate: u8, updater: WindowUpdater) {
    println!("init video pipline ...");
    gst::init().unwrap();

    let pipeline = gst::Pipeline::with_name("test-pipeline");

    let uridecodebin = gst::ElementFactory::make("uridecodebin")
        .property_from_str("uri", &video_uri)
        .build()
        .expect("Could not create gst element.");

    let videoconvert = gst::ElementFactory::make("videoconvert")
        .build()
        .expect("Could not create gst element.");

    let videoscale = gst::ElementFactory::make("videoscale")
        .build()
        .expect("Could not create gst element.");

    let videorate = gst::ElementFactory::make("videorate")
        .property_from_str("max-rate", max_rate.to_string().as_str())
        .build()
        .expect("Could not create gst element.");

    let appsink = gst_app::AppSink::builder()
        .caps(
            &gst_video::VideoCapsBuilder::new()
                .format(gst_video::VideoFormat::Rgb)
                .width(width as i32)
                .pixel_aspect_ratio(gst::Fraction::new(1, 1))
                .build(),
        )
        .build();

    pipeline
        .add_many([
            &uridecodebin,
            &videorate.upcast_ref(),
            &videoconvert.upcast_ref(),
            &videoscale.upcast_ref(),
            &appsink.upcast_ref(),
        ])
        .unwrap();

    let sink_pad = videorate.static_pad("sink").unwrap();
    uridecodebin.connect_pad_added(move |_, src_pad| {
        // get pad info
        let pad_caps = src_pad.current_caps().unwrap();
        let pad_struct = pad_caps.structure(0).unwrap();
        let pad_type = pad_struct.name();
    
        // skip not video pad
        if pad_type.starts_with("video/") {
            src_pad.link(&sink_pad).expect("Can't link uridecodebin with videorate!");
        }
    });
    gst::Element::link_many([&videorate, &videoconvert, &videoscale, &appsink.upcast_ref()])
        .expect("Many elements could not be linked.");

    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |appsink| {
                let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;

                let caps = sample.caps().unwrap().structure(0).unwrap();
                let v_height = caps.get::<i32>("height").unwrap();
                let v_width = caps.get::<i32>("width").unwrap();
                // println!("height {}", v_height);
                // println!("width {}", v_width);

                let buffer = sample.buffer_owned().unwrap(); // Probably copies!

                let video_info =
                    gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgb, v_width as u32, v_height as u32)
                        .build()
                        .expect("couldn't build video info!");
                let video_frame = gst_video::VideoFrame::from_buffer_readable(buffer, &video_info).unwrap();

                let slint_frame = try_gstreamer_video_frame_to_pixel_buffer(&video_frame)
                    .expect("Unable to convert the video frame to a slint video frame!");

                updater.update_video_frame(slint_frame);

                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );

    println!("init video pipline ... done");
    println!("starting video pipline ...");
    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    println!("starting video pipline ... OK");
}
