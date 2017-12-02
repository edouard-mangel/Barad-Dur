from flask import Flask
from flask import render_template
import subprocess
import credentials


app = Flask(__name__)
ipAddress = '192.168.1.19'
fileName = 'sample_videoSalon3.avi'
ffmpegPath = '/home/edouard/Softs/ffmpeg/ffmpeg'


bashRecord = ffmpegPath + ' -i rtsp://' + credentials.login() + ipAddress + str(" -vcodec copy -acodec copy ") + fileName

process = subprocess.Popen(bashRecord.split(), stdout=subprocess.PIPE)

@app.route("/")
def hello():
    return render_template('index.html', title='bb')

print(bashRecord)

# app.run(host='0.0.0.0:8080')
