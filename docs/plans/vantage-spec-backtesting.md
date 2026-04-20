# 🔭 Vantage: Spec for Backtesting

## 👤 User Story
As a Trader, I want to backtest against volatile markets, so that I can validate my trading strategies before risking real capital.

## ❓ So What?
What business problem does this solve? Traders currently lack a safe environment to test strategies against historical data, leading to higher risk and potential losses. Providing a backtesting feature allows traders to refine their algorithms, increasing confidence and user retention on the platform.

## 📏 Metric Definition
- **Success:** The backtesting engine can process 1 year of tick data in under 5 seconds.
- **Success:** The system handles missing or NaN data points without crashing.
- **Success:** The system correctly outputs a comprehensive CSV report of trades and performance.

## 🔍 Gap Analysis
Currently, traders have to use external tools (like Python/Pandas or dedicated platforms like TradingView) to backtest strategies, which requires exporting our data and managing multiple environments. By integrating backtesting natively, we reduce friction and keep users within our ecosystem. Standard libraries (e.g., `serde`, `csv`) can be utilized for report generation.

## ✅ Acceptance Criteria
- Must handle NaN data without panicking.
- Must output a CSV report containing trade history, profit/loss, and key metrics.

## 🚫 Out of Scope
- Real-time execution (Phase 2).
