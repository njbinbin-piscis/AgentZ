---
name: "healthcheck"
description: "Track water intake and sleep in a local JSON file with small Node.js commands. Use when the user logs drinking water, going to sleep, or waking up, or asks for stats."
description_zh: "用本地 JSON 文件记录饮水和睡眠，并查看统计。"
version: "1.0.0"
source: agentz
---

# Health Tracker

Simple tracking for water intake and sleep using JSON file.

## Data Format

File: `health-data.json` in the current working directory (run the commands from there).

```json
{
  "water": [{"time": "ISO8601", "cups": 2}],
  "sleep": [{"time": "ISO8601", "action": "sleep|wake"}]
}
```

## Add Water Record

When user says "drink X cups" or similar:

```bash
node -e "const fs=require('fs');const f='./health-data.json';let d={water:[],sleep:[]};try{d=JSON.parse(fs.readFileSync(f))}catch(e){}d.water.push({time:new Date().toISOString(),cups:CUPS});fs.writeFileSync(f,JSON.stringify(d));console.log('Recorded: '+CUPS+' cups')"
```

Replace `CUPS` with number from user input.

## Add Sleep Record

When user says "going to sleep":

```bash
node -e "const fs=require('fs');const f='./health-data.json';let d={water:[],sleep:[]};try{d=JSON.parse(fs.readFileSync(f))}catch(e){}d.sleep.push({time:new Date().toISOString(),action:'sleep'});fs.writeFileSync(f,JSON.stringify(d));console.log('Recorded: sleep')"
```

## Add Wake Record

When user says "woke up":

```bash
node -e "const fs=require('fs');const f='./health-data.json';let d={water:[],sleep:[]};try{d=JSON.parse(fs.readFileSync(f))}catch(e){}const last=d.sleep.filter(s=>s.action==='sleep').pop();d.sleep.push({time:new Date().toISOString(),action:'wake'});fs.writeFileSync(f,JSON.stringify(d));if(last){const h=((new Date()-new Date(last.time))/3600000).toFixed(1);console.log('Slept: '+h+' hours')}else{console.log('Recorded: wake')}"
```

## View Stats

When user asks for stats:

```bash
node -e "const fs=require('fs');const f='./health-data.json';let d={water:[],sleep:[]};try{d=JSON.parse(fs.readFileSync(f))}catch(e){}console.log('Water:',d.water.length,'records');console.log('Sleep:',d.sleep.length,'records');const today=d.water.filter(w=>new Date(w.time).toDateString()===new Date().toDateString());console.log('Today:',today.reduce((s,w)=>s+w.cups,0),'cups')"
```

## Update Record

To update last water entry:

```bash
node -e "const fs=require('fs');const f='./health-data.json';let d=JSON.parse(fs.readFileSync(f));d.water[d.water.length-1].cups=NEW_CUPS;fs.writeFileSync(f,JSON.stringify(d));console.log('Updated')"
```

## Delete Record

To delete last water entry:

```bash
node -e "const fs=require('fs');const f='./health-data.json';let d=JSON.parse(fs.readFileSync(f));d.water.pop();fs.writeFileSync(f,JSON.stringify(d));console.log('Deleted')"
```

## Notes

- Uses Node.js built-in modules only
- File auto-created if missing
- All timestamps in ISO8601 format