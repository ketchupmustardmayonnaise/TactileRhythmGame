use esp_hal::gpio::{Level, Output, OutputConfig, OutputPin};

pub struct PowerConfig<CmEn, Pw12En> {
    pub cm_en: CmEn,
    pub pw12_en: Pw12En,
}

pub struct Power<'d> {
    cm_en: Output<'d>,
    pw12_en: Output<'d>,
}

impl<'d> Power<'d> {
    pub fn new<CmEn, Pw12En>(config: PowerConfig<CmEn, Pw12En>) -> Self
    where
        CmEn: OutputPin + 'd,
        Pw12En: OutputPin + 'd,
    {
        let cm_en = Output::new(config.cm_en, Level::Low, OutputConfig::default());
        let pw12_en = Output::new(config.pw12_en, Level::Low, OutputConfig::default());

        Self { cm_en, pw12_en }
    }

    pub fn enable_cm(&mut self) {
        self.cm_en.set_high();
    }

    pub fn disable_cm(&mut self) {
        self.cm_en.set_low();
    }

    pub fn enable_pw12(&mut self) {
        self.pw12_en.set_low();
    }

    pub fn disable_pw12(&mut self) {
        self.pw12_en.set_high();
    }
}
