//! Demo playback analysis and statistics generation.

use crate::ticcmd::DemoTicCmd;

/// Statistics derived from a demo.
#[derive(Debug, Default, PartialEq)]
pub struct DemoStats {
    /// Total duration of the demo in tics.
    pub total_tics: usize,
    /// Calculated Actions Per Minute (APM).
    pub actions_per_minute: f32,
}

/// Utility for analyzing player input data within a demo.
pub struct DemoAnalyzer;

impl DemoAnalyzer {
    /// Calculate `DemoStats` (like APM) from an iterator of demo tics.
    pub fn analyze<I>(tics: I) -> DemoStats
    where
        I: Iterator<Item = Vec<DemoTicCmd>>,
    {
        let mut total_tics = 0;
        let mut actions = 0;

        for cmds in tics {
            total_tics += 1;
            for cmd in cmds {
                if cmd.forward_move != 0 || cmd.side_move != 0 || cmd.angle_turn != 0 || cmd.buttons != 0 {
                    actions += 1;
                }
            }
        }

        let minutes = total_tics as f32 / (35.0 * 60.0);
        let actions_per_minute = if minutes > 0.0 {
            actions as f32 / minutes
        } else {
            0.0
        };

        DemoStats {
            total_tics,
            actions_per_minute,
        }
    }

    /// Generate an ASCII sparkline representing input intensity over time.
    pub fn sparkline<I>(tics: I) -> String
    where
        I: Iterator<Item = Vec<DemoTicCmd>>,
    {
        let mut spark = String::new();
        for cmds in tics {
            let mut intense = false;
            for cmd in cmds {
                if cmd.forward_move != 0 || cmd.buttons != 0 {
                    intense = true;
                }
            }
            spark.push(if intense { '*' } else { '.' });
        }
        spark
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ticcmd::DemoTicCmd;

    #[test]
    fn test_analyze_apm() {
        let mut cmds = Vec::new();
        for i in 0..35 {
            cmds.push(vec![DemoTicCmd {
                forward_move: if i == 0 { 50 } else { 0 },
                side_move: 0,
                angle_turn: 0,
                buttons: u8::from(i == 0),
            }]);
        }
        let stats = DemoAnalyzer::analyze(cmds.into_iter());
        assert_eq!(stats.total_tics, 35);
        assert_eq!(stats.actions_per_minute.round() as u32, 60);
    }
}
