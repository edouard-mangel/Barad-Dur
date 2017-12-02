loginName = 'admin'
password = 'password'

def login(): 
 	if len(loginName)>0 and len(password) > 0 :
 		return loginName + ':' + password + '@'
 	else: 
 		return ""