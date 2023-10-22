async function getSettings() {
    try {
        const response = await fetch("http://127.0.0.1:{{ .PORT }}/config", {
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

async function saveSettings(settings) {
    await fetch("http://127.0.0.1:{{ .PORT }}/config", {
        method: "POST",
        headers: {
            "Content-Type": "application/json"
        },
        body: JSON.stringify(settings)
    });
}

async function saveToFile() {
    await fetch("http://127.0.0.1:{{ .PORT }}/saveconfig", {
        method: "POST",
        headers: {
            "Content-Type": "application/json"
        },
    });
}

function errorModal(message) {
    let modal = new bootstrap.Modal(document.getElementById("error_modal"));
    let modalText = document.getElementById("error_modal_text");
    modalText.textContent = message;
    modal.show();
}

function hideErrorModal() {
    let modal = new bootstrap.Modal(document.getElementById("error_modal"));
    modal.hide();
}

function updateSubmitMessage(message, type) {
    let msg = document.getElementById("submit-message");
    msg.classList.forEach((item) => {
        msg.classList.remove(item);
    });
    msg.classList.add("mt-2", type);
    msg.textContent = message;
}

async function updateSettingsFields(settings) {
    document.getElementById("twitch_client_id").value = settings.twitch_client_id;
    document.getElementById("twitch_client_secret").value = settings.twitch_client_secret;
    document.getElementById("twitch_username").value = settings.twitch_username;

    if (settings.soundcloud_enabled) {
        document.getElementById("soundcloud_enabled").checked = settings.soundcloud_enabled;
    }

    document.getElementById("soundcloud_oauth").value = settings.soundcloud_oauth;

    if (settings.spotify_enabled) {
        document.getElementById("spotify_enabled").checked = settings.spotify_enabled;
    }
    
    document.getElementById("spotify_client_id").value = settings.spotify_client_id;
    document.getElementById("spotify_client_secret").value = settings.spotify_client_secret;

    document.querySelector("#twitch_client_id").addEventListener("change", function() {
        settings.twitch_client_id = this.value;
    });

    document.querySelector("#twitch_client_secret").addEventListener("change", function() {
        settings.twitch_client_secret = this.value;
    });

    document.querySelector("#twitch_username").addEventListener("change", function() {
        settings.twitch_username = this.value;
    });

    document.querySelector("#soundcloud_enabled").addEventListener("click", function() {
        settings.soundcloud_enabled = !settings.soundcloud_enabled;
        document.getElementById("soundcloud_enabled").checked = settings.soundcloud_enabled;
    });
    
    document.querySelector("#soundcloud_oauth").addEventListener("change", function() {
        settings.soundcloud_oauth = this.value;
    });

    document.querySelector("#spotify_enabled").addEventListener("click", function() {
        settings.spotify_enabled = !settings.spotify_enabled;
        document.getElementById("spotify_enabled").checked = settings.spotify_enabled;
    });

    document.querySelector("#spotify_client_id").addEventListener("change", function() {
        settings.spotify_client_id = this.value;
    });

    document.querySelector("#spotify_client_secret").addEventListener("change", function() {
        settings.spotify_client_secret = this.value;
    });
}

document.addEventListener("DOMContentLoaded", async function() {
    
    document.querySelector("#error_modal_close").addEventListener("click", function() {
        hideErrorModal();
    });

    document.querySelector("#error_modal .modal-header button").addEventListener("click", function() {
        hideErrorModal();
    });

    document.querySelector("#save_settings").addEventListener("click", async function() {
        if (settings == null) {
            errorModal("Unable to save settings. Please ensure the server is running and try again.");
            return 
        }

        if (settings.spotify_enabled) {
            if (settings.spotify_client_id == "" || settings.spotify_client_secret == "") {
                updateSubmitMessage("Please enter a Spotify Client ID and Client Secret before enabling Spotify", "text-danger");
                return;
            }
            
        }

        if (settings.soundcloud_enabled) {
            if (settings.soundcloud_oauth == "") {
                updateSubmitMessage("Please enter a SoundCloud OAuth token before enabling SoundCloud", "text-danger");
                return;
            }
        }

        try {
            await saveSettings(settings);
        } catch (error) {
            errorModal("Unable to save settings. Please ensure the server is running and try again.");
            return;
        }
        updateSubmitMessage("Settings saved successfully", "text-success");
    });

    document.querySelector("#save_settings").addEventListener("click", async function() {
        await saveSettings(settings);
    });

    document.querySelector("#save_to_file").addEventListener("click", async function() {
        if (settings == null) {
            errorModal("Unable to save settings. Please ensure the server is running and try again.");
            return 
        }

        if (settings.spotify_enabled) {
            if (settings.spotify_client_id == "" || settings.spotify_client_secret == "") {
                updateSubmitMessage("Please enter a Spotify Client ID and Client Secret before enabling Spotify", "text-danger");
                return;
            }
            
        }

        if (settings.soundcloud_enabled) {
            if (settings.soundcloud_oauth == "") {
                updateSubmitMessage("Please enter a SoundCloud OAuth token before enabling SoundCloud", "text-danger");
                return;
            }
        }

        try {
            await saveSettings(settings);
            await saveToFile();
        } catch (error) {
            errorModal("Unable to save settings. Please ensure the server is running and try again.");
            return;
        }
        updateSubmitMessage("Settings saved to file successfully", "text-success");
    });

    let settings = await getSettings();

    if (settings == null) {
        errorModal("Unable to load settings. Please ensure the server is running and try again.");
    } else {
        await updateSettingsFields(settings);
    }
});