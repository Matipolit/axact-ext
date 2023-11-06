import { h, render } from "https://unpkg.com/preact?module";
import htm from "https://unpkg.com/htm?module";

const html = htm.bind(h);

function CPUS(props) {
  return html`
    <div>
      ${props.cpus.map((cpu) => {
        return html`<div class="bar">
          <div class="bar-inner" style="width: ${cpu}%"></div>
          <label>${cpu.toFixed(2)}%</label>
        </div>`;
      })}
    </div>
  `;
}

function RAM(props) {
  const ram = props.ram;
  const used_percent = (ram.used / ram.total)*100;
  return html`
    <div>
      <div class="bar">
        <div class="bar-inner" style="width: ${used_percent}%"></div>
        <label>${(ram.used*0.000000001).toFixed(2)} GB</label>
      </div>
    </div>
    `;
}


let url_cpus = new URL("/realtime/cpus", window.location.href);
url_cpus.protocol = url_cpus.protocol.replace("http", "ws");

const cpus_div = document.getElementById("cpus");
const ram_div = document.getElementById("ram");

let ws_cpu = new WebSocket(url_cpus.href);
ws_cpu.onmessage = (cpu_ev) => {
  let cpu_json = JSON.parse(cpu_ev.data);
  render(html`<${CPUS} cpus=${cpu_json}></${CPUS}>`, cpus_div);
};


let url_ram= new URL("/realtime/ram", window.location.href);
url_ram.protocol = url_ram.protocol.replace("http", "ws");

let ws_ram = new WebSocket(url_ram.href);
ws_ram.onmessage = (ram_ev) => {
  let ram_json = JSON.parse(ram_ev.data);
  render(html`<${RAM} ram=${ram_json}></${RAM}>`, ram_div);
};


