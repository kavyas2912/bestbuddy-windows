export const publishers: Record<string,string> = {all:'Recommended sources',yahoo:'Yahoo Finance',moneycontrol:'Moneycontrol',bbc:'BBC',guardian:'The Guardian',ap:'Associated Press',reuters:'Reuters',hindu:'The Hindu',express:'Indian Express',ndtv:'NDTV',cnbc:'CNBC',nasa:'NASA',nature:'Nature',verge:'The Verge',espn:'ESPN',variety:'Variety'};

Object.assign(publishers,{"toi": "Times of India", "hindustantimes": "Hindustan Times", "indiatoday": "India Today", "et": "Economic Times", "businessstandard": "Business Standard", "mint": "Mint", "loksatta": "Loksatta", "lokmat": "Lokmat", "sakal": "Sakal", "telegraph": "The Telegraph India", "anandabazar": "Anandabazar Patrika", "newindianexpress": "The New Indian Express", "thanthi": "Daily Thanthi", "manorama": "Malayala Manorama", "mathrubhumi": "Mathrubhumi", "deccanherald": "Deccan Herald", "prajavani": "Prajavani", "deccanchronicle": "Deccan Chronicle", "eenadu": "Eenadu", "sakshi": "Sakshi", "gujaratsamachar": "Gujarat Samachar", "divyabhaskar": "Divya Bhaskar", "npr": "NPR", "cnn": "CNN", "abc": "ABC News", "sky": "Sky News", "itv": "ITV News", "independent": "The Independent", "abcau": "ABC Australia", "sbs": "SBS News", "smh": "Sydney Morning Herald", "nine": "9News", "cbc": "CBC", "ctv": "CTV News", "globalnews": "Global News", "globe": "Globe and Mail", "tagesschau": "Tagesschau", "spiegel": "Der Spiegel", "zeit": "Die Zeit", "nhk": "NHK World", "japantimes": "Japan Times", "mainichi": "Mainichi", "asahi": "Asahi Shimbun"},{aljazeera:'Al Jazeera',dw:'DW',google:'Google News',msn:'MSN News',cnbctv18:'CNBC-TV18',etnow:'ET Now'});
const countries:Record<string,string[]>={"india": ["hindu", "express", "ndtv", "toi", "hindustantimes", "indiatoday"], "united states": ["ap", "reuters", "npr", "cnn", "abc"], "united kingdom": ["bbc", "guardian", "sky", "itv", "independent"], "australia": ["abcau", "sbs", "guardian", "smh", "nine"], "canada": ["cbc", "ctv", "globalnews", "globe", "reuters"], "germany": ["dw", "tagesschau", "spiegel", "zeit", "reuters"], "japan": ["nhk", "japantimes", "mainichi", "asahi", "reuters"]};
const regions:Record<string,string[]>={"maharashtra": ["loksatta", "lokmat", "sakal"], "west bengal": ["telegraph", "anandabazar"], "tamil nadu": ["hindu", "newindianexpress", "thanthi"], "kerala": ["manorama", "mathrubhumi"], "karnataka": ["deccanherald", "prajavani"], "telangana": ["deccanchronicle", "eenadu"], "andhra pradesh": ["eenadu", "sakshi"], "gujarat": ["gujaratsamachar", "divyabhaskar"]};
const specialists:Record<string,string[]>={"science": ["bbc", "nasa", "nature", "reuters", "guardian"], "ai-tech": ["bbc", "verge", "nature", "reuters", "guardian"], "business-world": ["bbc", "reuters", "cnbc", "yahoo", "guardian"]};
export function countryKey(country:string):string {const key=country.trim().toLowerCase();return ({in:'india',us:'united states',usa:'united states',uk:'united kingdom',gb:'united kingdom',au:'australia',ca:'canada',de:'germany',jp:'japan'} as Record<string,string>)[key]||key;}
export function sourceIds(category:string,country='India',region=''):string[] {
 const key=countryKey(country),home=countries[key]||['bbc','reuters','ap','guardian','aljazeera'];
 let ids:string[];
 if(category==='world')ids=['bbc','reuters','ap','guardian','aljazeera'];
 else if(specialists[category])ids=specialists[category];
 else if(category==='business'||category==='business-national')ids=key==='india'?['et','mint','businessstandard','moneycontrol','yahoo','cnbctv18','etnow','hindu','express','bbc']:['bbc','reuters','yahoo',...(key==='united states'?['cnbc']:[]),...home];
 else ids=['bbc',...((category==='local'||category==='regional')&&key==='india'?regions[region.trim().toLowerCase()]||[]:[]),...home];
 return ['all',...new Set([...ids,...(category==='sports'?['espn']:category==='entertainment'?['variety']:[]),'google','msn'])];
}
export function newsScope(category:string,country:string,region:string,city:string):string {
 if(['world','business-world'].includes(category))return 'Worldwide';
 if(['science','ai-tech'].includes(category))return 'Global research';
 return category==='local'?[city,region,country].filter(Boolean).join(', '):category==='regional'?[region,country].filter(Boolean).join(', '):country;
}
export function quietDuration(text:string):number|null {
 const words:Record<string,number>={a:1,an:1,one:1,two:2,three:3,four:4,five:5,ten:10,fifteen:15,twenty:20,thirty:30,sixty:60};
 const normalized=text.toLowerCase().replace(/half an? hour/g,'30 minutes').replace(/a quarter of an? hour/g,'15 minutes');
 const match=normalized.match(/(?:keep quiet|be quiet|stay quiet|quiet mode|quiet|shut up|silence|mute yourself)(?:\s+(?:please|for))*\s+(\d+(?:\.\d+)?|a|an|one|two|three|four|five|ten|fifteen|twenty|thirty|sixty)\s+(seconds?|minutes?|hours?)/);
 if(!match)return null;
 const count=Number(match[1])||words[match[1]];
 const duration=count*(match[2].startsWith('hour')?3600000:match[2].startsWith('minute')?60000:1000);
 return Number.isFinite(duration)&&duration>0?duration:null;
}
