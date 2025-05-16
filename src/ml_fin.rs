pub fn moving_average(prices: &[f64], window_size: usize) -> Option<f64> {
    if prices.len() < window_size {
        return None;
    }
    let sum: f64 = prices[prices.len() - window_size..].iter().sum();
    Some(sum / window_size as f64)
}
