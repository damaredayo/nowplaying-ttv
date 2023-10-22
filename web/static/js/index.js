document.addEventListener("DOMContentLoaded", async function() {
    async function getStatus() {
        try {
            const response = await fetch("http://127.0.0.1:{{ .PORT }}/status", {
                method: "GET",
                headers: {
                    "Content-Type": "application/json"
                }
            });
    
            if (!response.ok) {
                throw new Error(`HTTP error! status: ${response.status}`);
            }
    
            const json = await response.json();
            return json;
        } catch (error) {
            console.error(error);
        }
    }

    async function restart() {
        await fetch("http://127.0.0.1:{{ .PORT }}/restart", {
            method: "GET",
        });
    }

    async function updateStatusFields() {
        let status = await getStatus();
        let cpu_usage = document.getElementById("cpu_usage");
        let cpu_usage_text = document.getElementById("cpu_usage_text");
        let memory_usage = document.getElementById("memory_usage");
        let memory_usage_text = document.getElementById("memory_usage_text");

        cpu_usage_text.textContent = status.cpu_usage.toFixed(3) + "%";
        cpu_usage.style.width = status.cpu_usage + "%";

        let memory_percent = status.memory_usage / status.memory_total * 100;
        let memory_usage_mb = status.memory_usage / 1024 / 1024;
        let memory_total_mb = status.memory_total / 1024 / 1024;
        memory_usage_text.textContent = memory_percent.toFixed(2) + "%" + " (" + memory_usage_mb.toFixed(2) + " MB / " + memory_total_mb.toFixed(2) + " MB)";
        memory_usage.style.width = memory_percent + "%";
    }

    document.querySelector("#restart").addEventListener("click", async function() {
        await restart();
    });

    setInterval(async () => {
        await updateStatusFields();
    }, 10000);

    await updateStatusFields();
});