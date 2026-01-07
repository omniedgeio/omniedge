export namespace bridge {
	
	export class LoginResult {
	    success: boolean;
	    message: string;
	    token?: string;
	    refresh_token?: string;
	
	    static createFrom(source: any = {}) {
	        return new LoginResult(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.success = source["success"];
	        this.message = source["message"];
	        this.token = source["token"];
	        this.refresh_token = source["refresh_token"];
	    }
	}
	export class NetworkInfo {
	    id: string;
	    name: string;
	    ip_range: string;
	
	    static createFrom(source: any = {}) {
	        return new NetworkInfo(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.id = source["id"];
	        this.name = source["name"];
	        this.ip_range = source["ip_range"];
	    }
	}

}

