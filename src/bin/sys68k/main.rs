use std::{
    fmt::Debug,
    fs::File,
    io::{self, Read},
    net::{TcpListener, TcpStream, ToSocketAddrs},
    path::PathBuf,
};

use clap::Parser;
use gdb::SystemTarget;
use gdbstub::{
    common::Signal,
    conn::{Connection, ConnectionExt},
    stub::{
        run_blocking::{BlockingEventLoop, Event, WaitForStopReasonError},
        DisconnectReason, GdbStub, SingleThreadStopReason,
    },
    target::Target,
};
use system68k::sys::System;

mod gdb;

fn wait_for_gdb_connection<S: ToSocketAddrs + Debug>(sockaddr: S) -> io::Result<TcpStream> {
    eprintln!("Waiting for a GDB connection on {:?}...", sockaddr);
    let sock = TcpListener::bind(sockaddr)?;
    let (stream, addr) = sock.accept()?;

    // Blocks until a GDB client connects via TCP.
    // i.e: Running `target remote localhost:<port>` from the GDB prompt.
    eprintln!("Debugger connected from {}", addr);
    Ok(stream) // `TcpStream` implements `gdbstub::Connection`
}

struct GdbEventLoop;

impl BlockingEventLoop for GdbEventLoop {
    type Target = SystemTarget;
    type Connection = TcpStream;
    type StopReason = SingleThreadStopReason<u32>;

    fn wait_for_stop_reason(
        target: &mut Self::Target,
        conn: &mut Self::Connection,
    ) -> Result<
        Event<Self::StopReason>,
        WaitForStopReasonError<
            <Self::Target as Target>::Error,
            <Self::Connection as Connection>::Error,
        >,
    > {
        let mut tick = 0;
        while !target.cpu().is_stopped() {
            // Poll TCP conn every 1024 ticks for new data
            if (tick % 1024) == 0 {
                if conn.peek().map(|b| b.is_some()).unwrap_or(true) {
                    let byte = (conn as &mut dyn ConnectionExt<Error = io::Error>)
                        .read()
                        .map_err(WaitForStopReasonError::Connection)?;
                    return Ok(Event::IncomingData(byte));
                }
            }
            if target.step() {
                return Ok(Event::TargetStopped(SingleThreadStopReason::SwBreak(())));
            }
            tick += 1;
        }

        Ok(Event::TargetStopped(SingleThreadStopReason::Terminated(
            Signal::SIGSTOP,
        )))
    }

    fn on_interrupt(
        target: &mut Self::Target,
    ) -> Result<Option<Self::StopReason>, <Self::Target as Target>::Error> {
        Ok(Some(SingleThreadStopReason::Signal(Signal::SIGINT)))
    }
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to ROM file to load
    #[arg(value_name = "ROM")]
    file: PathBuf,

    /// Enable GDB remote debugging on address (e.g. localhost:5050)
    #[arg(short, long, value_name = "ADDRESS")]
    debug: Option<String>,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let mut rom = Vec::new();
    File::open(args.file)?.read_to_end(&mut rom)?;

    let mut sys = System::new(rom);
    sys.reset();

    let mut sys = SystemTarget::new(sys);

    if let Some(sockaddr) = args.debug {
        let conn = wait_for_gdb_connection(sockaddr)?;
        let debugger = GdbStub::new(conn);
        match debugger.run_blocking::<GdbEventLoop>(&mut sys) {
            Ok(reason) => match reason {
                DisconnectReason::Disconnect => {}

                DisconnectReason::TargetExited(code) => {
                    todo!()
                }

                DisconnectReason::TargetTerminated(code) => {
                    todo!()
                }

                DisconnectReason::Kill => {
                    todo!()
                }
            },

            Err(e) => {
                eprintln!("{e:?}");
            }
        };
    }

    while !sys.cpu().is_stopped() {
        sys.step();
    }

    Ok(())
}
