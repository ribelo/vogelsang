use chrono::{DateTime, Utc};

#[derive(Clone, Debug)]
pub struct Candle {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: Option<f64>,
    pub time: DateTime<Utc>,
    pub symbol: String,
}

#[derive(Clone, Debug, Default)]
pub struct Candles {
    pub symbol: String,
    pub open: Vec<f64>,
    pub high: Vec<f64>,
    pub low: Vec<f64>,
    pub close: Vec<f64>,
    pub volume: Option<Vec<f64>>,
    pub time: Vec<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct CandlesIterator<'a> {
    candles: &'a Candles,
    idx: usize,
}

pub trait CandlesExt {
    fn candles(&self) -> Candles;
}

impl Candles {
    pub fn len(&self) -> usize {
        self.time.len()
    }
    pub fn get(&self, index: usize) -> Option<Candle> {
        if index < self.time.len() {
            let symbol = &self.symbol;
            let open = self.open[index];
            let high = self.high[index];
            let low = self.low[index];
            let close = self.close[index];
            let volume = self.volume.as_ref().map(|x| x[index]);
            let time = self.time[index];

            Some(Candle {
                symbol: symbol.clone(),
                open,
                high,
                low,
                close,
                volume,
                time,
            })
        } else {
            None
        }
    }
    pub fn last(&self) -> Option<Candle> {
        self.time.last().map(|time| Candle {
            symbol: self.symbol.clone(),
            open: *self.open.last().unwrap(),
            high: *self.high.last().unwrap(),
            low: *self.low.last().unwrap(),
            close: *self.close.last().unwrap(),
            volume: self.volume.as_ref().map(|xs| *xs.last().unwrap()),
            time: *time,
        })
    }
    pub fn is_empty(&self) -> bool {
        self.time.is_empty()
    }
    pub fn push(
        &mut self,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: Option<f64>,
        time: DateTime<Utc>,
    ) {
        self.open.push(open);
        self.high.push(high);
        self.low.push(low);
        self.close.push(close);
        if let Some(value) = volume {
            self.volume.as_mut().unwrap().push(value);
        };
        self.time.push(time);
    }
    pub fn to_vec(&self) -> Vec<Candle> {
        let mut xs = Vec::new();
        for i in 0..self.len() {
            xs.push(self.get(i).unwrap())
        }
        xs
    }
    pub fn iter(&self) -> CandlesIterator {
        CandlesIterator {
            candles: self,
            idx: 0,
        }
    }
}

impl Iterator for CandlesIterator<'_> {
    type Item = Candle;
    fn next(&mut self) -> Option<Self::Item> {
        let candle = self.candles.get(self.idx);
        self.idx += 1;
        candle
    }
}
