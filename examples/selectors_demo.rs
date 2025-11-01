use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;

use genapi_core::{GenApiError, NodeMap, RegisterIo};
use genicam::{Camera, GenicamError};

fn main() -> Result<(), Box<dyn Error>> {
    const XML: &str = r#"
        <RegisterDescription SchemaMajorVersion="1" SchemaMinorVersion="2" SchemaSubMinorVersion="3">
            <Enumeration Name="GainSelector">
                <Address>0x300</Address>
                <Length>2</Length>
                <AccessMode>RW</AccessMode>
                <EnumEntry Name="AnalogAll" Value="0" />
                <EnumEntry Name="DigitalAll" Value="1" />
            </Enumeration>
            <Integer Name="Gain">
                <Address>0x304</Address>
                <Length>2</Length>
                <AccessMode>RW</AccessMode>
                <Min>0</Min>
                <Max>48</Max>
                <pSelected>GainSelector</pSelected>
                <Selected>AnalogAll</Selected>
            </Integer>
        </RegisterDescription>
    "#;

    let model = genapi_xml::parse(XML)?;
    let nodemap = NodeMap::from(model);
    let transport = MockIo::with_registers(&[(0x300, vec![0, 0]), (0x304, vec![0, 20])]);
    let mut camera = Camera::new(transport, nodemap);

    println!("Selector demo:");
    println!("  GainSelector -> {}", camera.get("GainSelector")?);
    println!("  Gain -> {}", camera.get("Gain")?);

    println!("Switching selector to DigitalAll");
    camera.set("GainSelector", "DigitalAll")?;
    match camera.get("Gain") {
        Ok(value) => println!("  Gain -> {value}"),
        Err(GenicamError::GenApi(GenApiError::Unavailable(_))) => {
            println!("  Gain is unavailable for DigitalAll selector value");
        }
        Err(err) => return Err(err.into()),
    }

    Ok(())
}

struct MockIo {
    regs: RefCell<HashMap<u64, Vec<u8>>>,
}

impl MockIo {
    fn with_registers(entries: &[(u64, Vec<u8>)]) -> Self {
        let mut regs = HashMap::new();
        for (addr, data) in entries {
            regs.insert(*addr, data.clone());
        }
        MockIo {
            regs: RefCell::new(regs),
        }
    }
}

impl RegisterIo for MockIo {
    fn read(&self, addr: u64, len: usize) -> Result<Vec<u8>, GenApiError> {
        let regs = self.regs.borrow();
        let data = regs
            .get(&addr)
            .ok_or_else(|| GenApiError::Io(format!("read miss at 0x{addr:08X}")))?;
        if data.len() != len {
            return Err(GenApiError::Io(format!(
                "length mismatch at 0x{addr:08X}: expected {len}, have {}",
                data.len()
            )));
        }
        Ok(data.clone())
    }

    fn write(&self, addr: u64, data: &[u8]) -> Result<(), GenApiError> {
        self.regs.borrow_mut().insert(addr, data.to_vec());
        Ok(())
    }
}
