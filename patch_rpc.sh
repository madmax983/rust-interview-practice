#!/bin/bash
sed -i '/type CallbackMap/i \
impl Drop for RpcClient {\
    fn drop(\&mut self) {\
        let stream = self.write_stream.lock().unwrap();\
        let _ = stream.shutdown(std::net::Shutdown::Both);\
    }\
}\
' src/networking/rpc.rs

sed -i '/Benchmarking Note:/c \
// Benchmarking Note:\
// To benchmark this implementation, you would typically use the `criterion` crate.\
// Measure the overhead of `RpcRequest::serialize` vs raw byte allocation, and test the \
// throughput of the `RpcClient::call` loop compared to a standard `TcpStream::write_all`.\
// Use `std::hint::black_box` to ensure the optimizer does not eliminate empty remote payloads.' src/networking/rpc.rs
