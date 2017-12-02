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
        body = jsonify({'id': cameraId, 'isRecording': isRecording, 'command': bashCommands.GetRecordCommand(cameraId)})
        response = body
        response.status_code = 200

        if isRecording == True:
            return response
        elif isRecording == False:
            return response
        else:
            raise Exception('Parsing error')
    except Exception:
        response.status_code = 500

        return response


#app.run(host= '0.0.0.0')
