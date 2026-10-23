// Observe native WebGL ownership and allocation without changing any GL result.
async function observeGpuResources(page) {
  await page.addInitScript(()=>{
    const report={texturesCreated:0,framebuffersCreated:0,texturesDeleted:0,
      framebuffersDeleted:0,targets:[]};
    Object.defineProperty(window,'__solomonGpuResources',{value:report});
    const contexts=new WeakMap(),textures=new WeakMap(),framebuffers=new WeakMap();
    let nextTexture=0,nextFramebuffer=0;
    const state=gl=>{
      if(!contexts.has(gl))contexts.set(gl,{unit:gl.TEXTURE0,textures:new Map(),framebuffer:null});
      return contexts.get(gl);
    };
    const allocated=(gl,width,height)=>{
      const current=state(gl);
      const texture=textures.get(current.textures.get(current.unit));
      if(texture){texture.width=width;texture.height=height}
    };
    const wrap=(prototype,name,observe)=>{
      const original=prototype[name];
      if(!original)return;
      prototype[name]=function(...args){
        const result=Reflect.apply(original,this,args);
        observe(this,args,result);
        return result;
      };
    };
    for(const prototype of [WebGLRenderingContext.prototype,WebGL2RenderingContext.prototype]) {
      wrap(prototype,'createTexture',(gl,args,texture)=>{
        if(!texture)return;
        report.texturesCreated+=1;
        textures.set(texture,{id:++nextTexture,width:0,height:0,deleted:false});
      });
      wrap(prototype,'deleteTexture',(gl,[texture])=>{
        const tracked=textures.get(texture);
        if(tracked&&!tracked.deleted){tracked.deleted=true;report.texturesDeleted+=1}
      });
      wrap(prototype,'activeTexture',(gl,[unit])=>{state(gl).unit=unit});
      wrap(prototype,'bindTexture',(gl,[target,texture])=>{
        if(target===gl.TEXTURE_2D)state(gl).textures.set(state(gl).unit,texture);
      });
      wrap(prototype,'texImage2D',(gl,args)=>{
        if(args[0]!==gl.TEXTURE_2D||args[1]!==0)return;
        if(args.length>=9)allocated(gl,args[3],args[4]);
        else if(args.length===6)allocated(gl,args[5].naturalWidth||args[5].videoWidth||args[5].width,
          args[5].naturalHeight||args[5].videoHeight||args[5].height);
      });
      wrap(prototype,'texStorage2D',(gl,args)=>{
        if(args[0]===gl.TEXTURE_2D)allocated(gl,args[3],args[4]);
      });
      wrap(prototype,'createFramebuffer',(gl,args,framebuffer)=>{
        if(!framebuffer)return;
        report.framebuffersCreated+=1;
        const target={id:++nextFramebuffer,canvas:gl.canvas.id||'(unattached canvas)',
          texture:null,draws:0,deleted:false};
        framebuffers.set(framebuffer,target);
        report.targets.push(target);
      });
      wrap(prototype,'deleteFramebuffer',(gl,[framebuffer])=>{
        const target=framebuffers.get(framebuffer);
        if(target&&!target.deleted){target.deleted=true;report.framebuffersDeleted+=1}
        if(state(gl).framebuffer===framebuffer)state(gl).framebuffer=null;
      });
      wrap(prototype,'bindFramebuffer',(gl,[target,framebuffer])=>{
        if(target===gl.FRAMEBUFFER||target===gl.DRAW_FRAMEBUFFER)state(gl).framebuffer=framebuffer;
      });
      wrap(prototype,'framebufferTexture2D',(gl,[target,attachment,type,texture])=>{
        if(target!==gl.FRAMEBUFFER&&target!==gl.DRAW_FRAMEBUFFER)return;
        if(attachment!==gl.COLOR_ATTACHMENT0)return;
        const framebuffer=framebuffers.get(state(gl).framebuffer);
        if(framebuffer)framebuffer.texture=textures.get(texture)||null;
      });
      for(const [name,countIndex]of [['drawArrays',2],['drawElements',1]]) {
        wrap(prototype,name,(gl,args)=>{
          const target=framebuffers.get(state(gl).framebuffer);
          if(target&&args[countIndex]>0)target.draws+=1;
        });
      }
    }
  });
}

const gpuResources=page=>page.evaluate(()=>window.__solomonGpuResources);
module.exports={observeGpuResources,gpuResources};
