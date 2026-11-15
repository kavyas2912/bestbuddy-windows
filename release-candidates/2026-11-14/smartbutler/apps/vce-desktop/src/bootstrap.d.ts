declare module 'bootstrap/js/dist/modal' {
 export default class Modal {
 constructor(element:Element,options?:{backdrop?:boolean|'static';keyboard?:boolean;focus?:boolean});
 show():void;hide():void;dispose():void;
 }
}
