//! Headless harness for reproducible integration checks. Shares the production pipeline.
#[path="../engine.rs"]mod engine;
#[path="../jobs.rs"]mod jobs;
#[path="../export.rs"]mod export;
#[path="../caj.rs"]mod caj;
use std::{path::PathBuf,fs};
use serde_json::{Value,json};
fn run()->Result<Value,String>{let args:Vec<_>=std::env::args().collect();let req:Value=serde_json::from_slice(&fs::read(args.get(1).ok_or("Pass a request JSON file")?).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;let engine=engine::Engine{resources:std::env::var_os("BPDF_RESOURCES").map(PathBuf::from).unwrap_or_else(||PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources"))};if req["mode"]=="job"{let inputs=req["inputs"].as_array().ok_or("Missing inputs")?.iter().map(|v|PathBuf::from(v.as_str().unwrap_or_default())).collect::<Vec<_>>();let (outputs,warnings)=jobs::run(&engine,req["tool"].as_str().ok_or("Missing tool")?,&inputs,&PathBuf::from(req["output"].as_str().ok_or("Missing output")?),&req["options"],&engine::fresh_cancel(),&|_,_|{})?;Ok(json!({"outputs":outputs,"warnings":warnings}))}else{engine.request(req,&engine::fresh_cancel())}}
fn main(){match run(){Ok(v)=>println!("{}",json!({"ok":true,"result":v})),Err(e)=>{println!("{}",json!({"ok":false,"error":e}));std::process::exit(1)}}}
