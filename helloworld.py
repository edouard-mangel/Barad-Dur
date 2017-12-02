from bottle import route, run

@route('/hello')
def hello():
    return "<h1>Hello World!</h1>"

@route('/')

run(host='0.0.0.0', port=8080, debug=True)
