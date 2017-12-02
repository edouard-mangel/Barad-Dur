from flask import Flask
from flask import render_template
import subprocess
import credentials
import bashCommands

app = Flask(__name__)

@app.route("/")
def hello():
    return render_template('index.html', title='bb')

app.run(host='0.0.0.0:8080')
