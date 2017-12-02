from flask import Flask
from flask import render_template
from flask import jsonify

import subprocess
import credentials
import bashCommands

app = Flask(__name__)

@app.route('/', methods=['GET'])
def hello():
    return render_template('index.html')

@app.route('/<cameraId>/<isRecording>', methods=['POST'])
def toggle_camera_recording(cameraId, isRecording):
    try:
        cameraId = int(cameraId[-1]) - 1
        isRecording = (isRecording == 'true')
        body = jsonify({'id': cameraId, 'isRecording': isRecording})
        response = body
        response.status_code = 200
        bashCommands.ToggleRecord(cameraId)
    except Exception:
        response.status_code = 500
    finally:
        return response

#app.run(host= '0.0.0.0')
