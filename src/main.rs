#![allow(non_snake_case)]
use chrono::offset::Local;
use once_cell::sync::Lazy;
use std::{env, iter, mem, fs::OpenOptions, io::*, time::Duration};
use image::{DynamicImage, RgbaImage, codecs::png::PngEncoder, imageops::FilterType};
use windows::{
	core::*, Win32::{Foundation::*, Graphics::Gdi::*, Storage::Xps::*, UI::HiDpi::*, UI::WindowsAndMessaging::*}
};

const TITLE: Lazy<PCWSTR> = Lazy::new(|| {
	let var = env::var("PLAYBRIDGE_TITLE").unwrap_or(String::from("명일방주"));
	let vec: Vec<u16> = var.encode_utf16().chain(iter::once(0)).collect::<Vec<_>>();

	PCWSTR::from_raw(vec.as_ptr())
});
const QUICK: Lazy<bool> = Lazy::new(|| env::var("PLAYBRIDGE_QUICK").is_ok());
const DEBUG: Lazy<bool> = Lazy::new(|| env::var("PLAYBRIDGE_DEBUG").is_ok());

const CLASS: PCWSTR = w!("CROSVM_1"); // Note: Warning. May cause problems in the future.
const WIDTH: f32 = 1280.0;
const HEIGHT: f32 = 720.0;
const POLL: i32 = 1000 / 250;

fn main() {
	unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).unwrap() };

	let args: Vec<String> = env::args().collect();
	let command = args.join(" ");
	let timestamp = Local::now().format("%Y-%m-%d %H.%M.%S_%3f").to_string();

	if command.contains("connect") {
		println!("connected to Google Play Games Beta");
	} else if command.contains("devices") {
		println!("List of devices attached");
		if unsafe { FindWindowW(CLASS, TITLE) } != HWND(0) {
			println!("GooglePlayGamesBeta\tdevice")
		}
	} else if command.contains("shell getprop ro.build.version.release") {
		println!("14")
	} else if command.contains("shell am start -n") {
		let intent = args[7].parse::<String>().unwrap();
		let package = intent.split("/").next().unwrap();

		if unsafe { FindWindowW(CLASS, *TITLE) } == HWND(0) {
			_ = open::that(format!("googleplaygames://launch/?id={}", package));
		}
		
		println!("Starting: Intent {{ cmp={} }}", intent);
		println!("Warning: Activity not started, intent has been delivered to currently running top-most instance."); // Note: As PlayBridge does not call the intent directly, it needs to print warning.
	} else if command.contains("input tap") {
		let x = args[6].parse::<i32>().unwrap();
		let y = args[7].parse::<i32>().unwrap();

		input_tap(x, y);
	} else if command.contains("input swipe") {
		let x1 = args[6].parse::<i32>().unwrap();
		let y1 = args[7].parse::<i32>().unwrap();
		let x2 = args[8].parse::<i32>().unwrap();
		let y2 = args[9].parse::<i32>().unwrap();
		let dur = args[10].parse::<i32>().unwrap();

		input_swipe(x1, y1, x2, y2, dur);
	} else if command.contains("input keyevent 111") {
		input_keyevent(0x01);
	} else if command.contains("dumpsys window displays") || command.contains("wm size") {
		println!("{}", WIDTH as i32);
		println!("{}", HEIGHT as i32);
	} else if command.contains("exec-out screencap -p") {
		let image = capture();

		let mut stdout = stdout().lock();
		image.write_with_encoder(PngEncoder::new(&mut stdout)).unwrap();

		if *DEBUG {
			let file = format!("playbridge_debug/{}.png", timestamp);
			let path = std::path::Path::new(&file);
			let parent = path.parent().unwrap();

			std::fs::create_dir_all(parent).unwrap();
			image.save_with_format(path, image::ImageFormat::Png).unwrap();
		}
	} else if command.contains("am force-stop") {
		terminate();
	}

	if *DEBUG {
		let mut log = OpenOptions::new().create(true).append(true).open("playbridge.log").unwrap();
		_ = writeln!(log, "[{}] {}", timestamp, command);
	}
}

fn get_gpg_info() -> (HWND, i32, i32) {
	let hwnd = unsafe { FindWindowW(CLASS, *TITLE) };

	let mut client_rect = RECT::default();
	_ = unsafe { GetClientRect(hwnd, &mut client_rect) };
	
	(hwnd, (client_rect.right - client_rect.left) as i32, (client_rect.bottom - client_rect.top) as i32)
}

fn get_relative_point(x: i32, y: i32, w: i32, h: i32) -> isize {
	let nx = (x as f32 / WIDTH * w as f32).ceil() as isize;
	let ny = (y as f32 / HEIGHT * h as f32).ceil() as isize;

	ny << 16 | nx
}

fn input_tap(x: i32, y: i32) {
	let (hwnd, w, h) = get_gpg_info();
	let pos = get_relative_point(x, y, w, h);

	unsafe {
		_ = PostMessageA(hwnd, WM_LBUTTONDOWN, WPARAM(1), LPARAM(pos));
		_ = PostMessageA(hwnd, WM_LBUTTONUP, WPARAM(1), LPARAM(pos));
	}
}

fn input_swipe(x1: i32, y1: i32, x2: i32, y2: i32, dur: i32) {
	let (hwnd, w, h) = get_gpg_info();

	let time = (dur as f32 / POLL as f32).floor() as i32;
	let speed = if *QUICK { 10 } else  { 1 };
	let index = time * (speed - 1) / speed;

	let dx = ((x2 - x1) as f32) / time as f32;
	let dy = ((y2 - y1) as f32) / time as f32;

	unsafe {
		let mut cnt = 0;
		while cnt < time {
			let nx = x1 + (dx * cnt as f32) as i32;
			let ny = y1 + (dy * cnt as f32) as i32;
			let pos = get_relative_point(nx, ny, w, h);

			_ = PostMessageA(hwnd, WM_LBUTTONDOWN, WPARAM(1), LPARAM(pos));

			let wait = if cnt >= index { POLL * 1000000 } else { POLL * 1000000 / speed } as u32;

			spin_sleep::sleep(Duration::new(0, wait));
			cnt += 1;
		}

		let pos = get_relative_point(x2, y2, w, h);
		_ = PostMessageA(hwnd, WM_LBUTTONDOWN, WPARAM(1), LPARAM(pos));
		_ = PostMessageA(hwnd, WM_LBUTTONUP, WPARAM(1), LPARAM(pos));
	}
}

fn input_keyevent(keycode: i32) {
	let hwnd = unsafe { FindWindowW(CLASS, *TITLE) };

	let wparam = WPARAM(keycode as usize);
	let down = LPARAM((keycode << 16) as isize);
	let up = LPARAM((keycode << 16 | 1 << 30 | 1 << 31) as isize);

	unsafe {
		_ = PostMessageA(hwnd, WM_KEYDOWN, wparam, down);
		_ = PostMessageA(hwnd, WM_KEYUP, wparam, up);
	}
}

fn capture() -> DynamicImage {
	let hwnd = unsafe { FindWindowW(CLASS, *TITLE) };
	let swnd = unsafe { FindWindowExA(hwnd, HWND(0), s!("subWin"), PCSTR::null()) };
	
	let mut rect = RECT::default();
	_ = unsafe { GetWindowRect(swnd, &mut rect) };

	let width = rect.right - rect.left;
	let height = rect.bottom - rect.top;

	let mut buffer = vec![0u8; (width * height) as usize * 4];
	let mut info = BITMAPINFO {
		bmiHeader: BITMAPINFOHEADER {
			biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
			biWidth: width,
			biHeight: height,
			biPlanes: 1,
			biBitCount: 32,
			biCompression: 0,
			biSizeImage: 0,
			biXPelsPerMeter: 0,
			biYPelsPerMeter: 0,
			biClrUsed: 0,
			biClrImportant: 0,
		},
		bmiColors: [RGBQUAD::default(); 1],
	};

	unsafe {
		let dc = GetDC(hwnd);
		let cdc = CreateCompatibleDC(dc);
		let cbmp = CreateCompatibleBitmap(dc, width, height);

		SelectObject(cdc, cbmp);
		_ = PrintWindow(hwnd, cdc, PRINT_WINDOW_FLAGS(PW_CLIENTONLY.0 | PW_RENDERFULLCONTENT));
		GetDIBits(cdc, cbmp, 0, height as u32, Some(buffer.as_mut_ptr() as *mut _), &mut info, DIB_RGB_COLORS);
		
		_ = DeleteObject(cbmp);
		ReleaseDC(hwnd, dc);
		_ = DeleteDC(dc);
		_ = DeleteDC(cdc);
	}

	let mut chunks: Vec<Vec<u8>> = buffer.chunks(width as usize * 4).map(|x| x.to_vec()).collect();
	chunks.reverse();

	let rgba = chunks.concat().chunks_exact(4).take((width * height) as usize).flat_map(|bgra| [bgra[2], bgra[1], bgra[0], bgra[3]]).collect();
	let image = RgbaImage::from_vec(width as u32, height as u32, rgba).unwrap();
	let native = image::DynamicImage::ImageRgba8(image);
	
	native.resize_exact(WIDTH as u32, HEIGHT as u32, FilterType::Lanczos3)
}

fn terminate() {
	let hwnd = unsafe { FindWindowW(CLASS, *TITLE) };
	_ = unsafe { PostMessageA(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)) };
}