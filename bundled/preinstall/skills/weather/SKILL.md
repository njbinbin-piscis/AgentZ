---
name: "weather"
description: "Get current weather and forecasts from wttr.in and Open-Meteo, no API key needed. Use when the user asks about the weather."
description_zh: "通过 wttr.in 和 Open-Meteo 查询当前天气和预报，无需 API 密钥"
version: "1.0.0"
source: agentz
---

# Weather

Two free services, no API keys needed.

## wttr.in (primary)

```bash
curl -s "wttr.in/London?format=3"
curl -s "wttr.in/London?format=%l:+%c+%t+%h+%w"
curl -s "wttr.in/London?T"
```

Format codes: `%c` condition, `%t` temp, `%h` humidity, `%w` wind, `%l` location

Tips:
- URL-encode spaces: `wttr.in/New+York`
- Airport codes: `wttr.in/JFK`
- Units: `?m` (metric) `?u` (USCS)
- Today only: `?1`, Current only: `?0`

## Open-Meteo (fallback, JSON)

```bash
curl -s "https://api.open-meteo.com/v1/forecast?latitude=51.5&longitude=-0.12&current_weather=true"
```