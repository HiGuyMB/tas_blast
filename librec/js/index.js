function readFileAsync(file) {
    return new Promise((resolve, reject) => {
        let reader = new FileReader();

        reader.onload = () => {
            resolve(reader.result);
        };

        reader.onerror = reject;

        reader.readAsArrayBuffer(file);
    })
}

import("../pkg/index.js").then((librec) => {
    const textbox = document.getElementById("rect");
    const button = document.getElementById("convert");
    const error_div = document.getElementById("error");
    const input_file = document.getElementById("input_file");
    const input_json = document.getElementById("input_json");

    var last_conts = null;

    function update_conts() {
        if (last_conts === null) {
            return;
        }
        let converted;
        if (input_json.checked) {
            converted = librec.import_json(last_conts);
        } else {
            converted = librec.import_rect(last_conts);
        }
        try {
            textbox.value = converted;
        } catch (e) {
            output_div.innerText = "Error: " + e.toString();
        }
    }

    input_file.addEventListener('change', async (e) => {
        let conts = input_file.files[0];
        if (conts) {
            last_conts = new Uint8Array(await readFileAsync(conts));
            update_conts();
        }
    });

    input_json.addEventListener('change', (e) => {
        update_conts();
    });

    button.addEventListener('click', function(){
        let conts = textbox.value.replace(/\t/g, "    ");
        let converted;
        if (input_json.checked) {
            converted = librec.export_json(conts);
        } else {
            converted = librec.export_rect(conts);
        }

        let success = converted[0];
        converted = converted.slice(1);

        if (success) {
            downloadBlob(converted, 'generated_replay.tas.rec', 'application/octet-stream');
        } else {
            let error = new TextDecoder("utf-8").decode(converted);
            error_div.innerText = error;
        }
    });

    function downloadBlob(data, fileName, mimeType) {
        let blob = new Blob([data], {
            type: mimeType
        });
        let url = window.URL.createObjectURL(blob);
        downloadURL(url, fileName);
        setTimeout(function() {
            return window.URL.revokeObjectURL(url);
        }, 1000);
    }

    function downloadURL(data, fileName) {
        let a = document.createElement('a');
        a.href = data;
        a.download = fileName;
        document.body.appendChild(a);
        a.style = 'display: none';
        a.click();
        a.remove();
    }
});