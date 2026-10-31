// Isolated demonstration data for the promo. No live player state.
export const structures={command_center:2,mining_complex:2,smelter:2,shipyard:2,habitat:2,warehouse:2,agroplex:2,power_plant:2};
const base={parent:null,kind:'terrestrial',habitable:true,size:'large',environment:'terran',geology:'rich',special:null,
  structures:{},deposits:[],resource_slots:2,industrial_slots:3,infrastructure_slots:4,
  construction_time_mult:1,structure_build_time_mult:1,habitat_capacity_mult:1,population_growth_mult:1,
  population:.12,population_capacity:.5,provisions_mult:1};
export const bodies=[
  {...base,id:0,name:'Aster I',habitable:false,kind:'rocky',environment:'hostile',size:'medium',orbit:{radius:.29,angle:3.65,period:500}},
  {...base,id:1,name:'Aster Prime',structures,orbit:{radius:.51,angle:.68,period:1200},founding_camp:{profile:'verdant_lowlands',completed:['quarters','prospecting','storage','workforce']}},
  {...base,id:2,name:'Aster III',habitable:false,kind:'gas_giant',environment:'uninhabitable',size:'huge',orbit:{radius:.75,angle:2.42,period:2400}},
  {...base,id:3,name:'Aster IV',habitable:false,kind:'ice',environment:'uninhabitable',size:'large',orbit:{radius:.97,angle:5.65,period:4200}},
  {...base,id:4,parent:2,name:'Aster IIIa',habitable:false,kind:'rocky',environment:'uninhabitable',size:'small',orbit:{radius:.13,angle:1.1,period:300}},
];
export const system={id:'preview-home',name:'Aster',pos:{x:2000,y:3000},band:'rich',body_count:5};
