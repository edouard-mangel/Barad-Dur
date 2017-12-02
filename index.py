from flask import Flask
from flask import render_template
import subprocess
import credentials
import bashCommands

app = Flask(__name__)

testVar = bashCommands.GetCommand(1)

@app.route("/")
def hello():
    return render_template('index.html', title=testVar)

app.run(host='0.0.0.0')
