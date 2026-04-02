demo video - https://www.youtube.com/watch?v=qEZXpI9APYk



AnalBeats is a small Rust app that connects Beat Saber (through BSDataPuller) to Intiface/Buttplug toys.



It runs a local web dashboard where you can:



\- see connection status

\- pick a toy for hit and miss reactions (You can have different toys for each reaction, or use the same toy)

\- set mode/intensity/duration/cooldown

\- test hit/miss reactions

\- save config



\## What you need



\- Windows!!

\- \[Rust](https://www.rust-lang.org/tools/install)

\- \[Intiface Central](https://intiface.com/central/)

\- Beat Saber with BSDataPuller on your headset/PC setup (IDK IF OCCULUS WILL WORK IM TO LAZY TO TEST BUT SHOULD??)

\- a buttplug.io supported toy


\## How do I get the datapuller plugin for beatsaber???

https://github.com/Zagrios/bs-manager

Get version 1.36.2 (Or any other version that has DataPuller in the BS manager) 

donezo 

 (Im sure there is newer websocket plugins but thats the one I used. If you used a different one, change the url and verify that the names for the stuff is still right)



\## Run locally




git clone https://github.com/Blu3Be4ry/AnalBeats.git

cd AnalBeats

cargo run 




Open:



\- `http://127.0.0.1:3030`




## Project layout



\- `src/main.rs` – backend server + DataPuller listener + Buttplug control

\- `frontend/index.html` – dashboard page

\- `frontend/app.js` – dashboard logic

\- `frontend/style.css` – dashboard 💅styles⭐

\- `config.example.json` – example config



`config.json` is created locally when you run the app. If not, I fucked up. Try renaming config.example to config





JUST TO VERIFY!



1\. Start Intiface Central

2\. Connect your device(s) in Intiface

3\. Start Beat Saber + BSDataPuller

4\. Start AnalBeats

5\. In the dashboard:

&#x20;  - verify DataPuller websocket URL

&#x20;  - select toy for hit

&#x20;  - select toy for miss

&#x20;  - click Save Config

&#x20;  - test hit/miss buttons



\## DataPuller URL



Default:



\- `ws://127.0.0.1:2946/BSDataPuller/LiveData`



(reminder that there is also /MapData if you want to add other detections)


If Beat Saber is on Quest and your PC is reading over LAN, use:



\- `ws://<QUEST\_IP>:2946/BSDataPuller/LiveData`



Dont ask me how you get the ip idfk 



\## Notes



\- Toy selection is strict. If the selected toy is missing/disconnected, reactions fail until fixed.

\- Rotate mode only works on toys that support rotate.

\- If a mode is unsupported by your toy, test will return an error in the log.



\## Troubleshooting



\### Dashboard stuck on Loading

\- Check that `frontend/app.js` exists

\- Hard refresh browser (`Ctrl+F5`)

\- Open `http://127.0.0.1:3030/api/status` and confirm DataPuller is exposing himself...



\### No toys in dropdown

\- Make sure Intiface is running

\- Make sure device is connected in Intiface (not just powered on)



\### Hit/Miss test say toy is missing

\- Re-select toy in dropdown

\- Save config!!!!!!

\- Make sure that toy is still connected



\### Rotate does fuck all

\- Toy likely does not support rotate

\- Try vibrate or oscillate mode idk



