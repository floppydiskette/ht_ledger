mod ledger;

use std::io::{Read, Write};
use ht_timeparser::HTDate;
use rce::{CommandInterface, Invoker};
use crate::ledger::HLedger;

fn capture() -> (ht_cal::history::HistoryData, ht_cal::packet::PacketData) {
    let host = std::env::var("HT_HOST").unwrap_or("localhost".to_string());
    let port = 3926;

    // connect via tcp, send a single byte, and interpret the response as an rmp_serde::from_slice
    let mut stream = std::net::TcpStream::connect(format!("{}:{}", host, port)).unwrap();
    stream.write_all(&[0]).unwrap();
    let mut data = Vec::new();
    stream.read_to_end(&mut data).unwrap();
    let history: ht_cal::history::HistoryData = rmp_serde::from_slice(&data).unwrap();
    // reconnect
    let port = 3621;
    let mut stream = std::net::TcpStream::connect(format!("{}:{}", host, port)).unwrap();
    stream.write_all(&[0]).unwrap();
    let mut data = Vec::new();
    stream.read_to_end(&mut data).unwrap();
    let packet: ht_cal::packet::PacketData = rmp_serde::from_slice(&data).unwrap();
    (history, packet)
}

fn record() {
    let mut ledger = HLedger::load();
    let history = capture();
    ledger.import_from_htcal(&history.0, &history.1);
    ledger.save();
}

fn main() {
    let mut interface = CommandInterface::new(
        "ht_ledger",
        "huskitopian ledger program"
    );
    let a_date = interface.add_argument(Invoker::NWithoutInvoker(0), "YYYY-[GL][ZNASF]-DD");
    let c_record = interface.add_command(Invoker::Dash("r"), vec![], "records from time server into ledger");
    let c_print = interface.add_command(Invoker::Dash("p"), vec![a_date], "prints ledger");
    interface.finalise();

    let input = interface.go_and_print_usage_on_failure();
    if input.is_err() {
        println!("error: {:?}", input.err().unwrap());
        return;
    }

    let input = input.unwrap();
    if input.command == c_record {
        println!("recording...");
        record();
        println!("done!");
    } else if input.command == c_print {
        let ledger = HLedger::load();
        let day = HTDate::interpret_string(input.arguments.get(&a_date).unwrap()).unwrap();
        let records = ledger.collect(&day);
        let json = serde_json::to_string(&records).unwrap();
        println!("{}", json);
    }
}
