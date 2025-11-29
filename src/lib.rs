use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
use pyo3_asyncio::tokio::init_multi_thread_once;
use crate::net::client::client_connect;
mod net;

#[pyfunction]
fn connect(py: Python<'_>, url: String) -> PyResult<&PyAny> {
    pyo3_asyncio::tokio::future_into_py(py, async move {
        match client_connect(url).await {
            Ok(_) => Ok(()),
            Err(_) => Err(pyo3::exceptions::PyRuntimeError::new_err("connection failed")),
        }
    })
}

#[pymodule]
fn data_broker_client(py: Python<'_>, m: &PyModule) -> PyResult<()> {
    pyo3_asyncio::tokio::init_multi_thread_once();
    m.add_function(wrap_pyfunction!(connect, m)?)?;
    Ok(())
}
